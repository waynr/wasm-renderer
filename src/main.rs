use std::fs::File;
use std::io::prelude::*;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::atomic;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use std::thread;

use druid::widget::Painter;
use druid::{AppLauncher, Color, RenderContext, Widget, WidgetExt, WindowDesc};
use wasmer::{imports, Instance, MemoryView, Module, Store};

struct WasmDemoRunner {
    wasm_store: Store,
    module_instance: Instance,

    width: u32,
    height: u32,
    bytes_required: u64,

    frame_manager: FrameManager,

    state: State,
}

#[derive(Debug)]
enum State {
    Idle,
    Running,
}

#[derive(Debug)]
struct FrameManager {
    size: usize,
    frames: Vec<Frame>,
    last_updated: Option<Frame>,
}

impl FrameManager {
    fn new(size: usize) -> Self {
        Self {
            size,
            last_updated: None,
            frames: vec![
                Frame::new(size),
                Frame::new(size),
                Frame::new(size),
                Frame::new(size),
                Frame::new(size),
            ],
        }
    }

    fn get_free_frame(&mut self) -> std::result::Result<Frame, Box<dyn std::error::Error>> {
        let frame = self
            .frames
            .iter_mut()
            .find(|f| Frame::count(&f) == 1)
            .ok_or("couldn't find free frame")?;

        Ok(frame.clone())
    }
}

#[derive(Debug)]
struct Frame {
    ptr: NonNull<InnerFrame>,
    phantom: PhantomData<InnerFrame>,
}

// following the rustinomicon guide for implementing Arc: https://doc.rust-lang.org/nomicon/arc-mutex/arc-base.html
//
// the goal is to satisfy the constraints on image::Handle::from_pixels:
//      impl AsRef<[u8]> + Send + Sync + 'static,
// unfortunately I can't just wrap a Vec<u8> in Arc<Mutex<T>> because of the AsRef<[u8]> constraint
// and I haven't been able to figure out how to return &[u8] from a type protected by Arc<Mutex<T>>
//
// this is ultimately intended to serve the purpose of not allocating a new Vec<u8> every time i
// want to pass a wasm-generated pixel buffer to the iced library
impl Frame {
    fn new(size: usize) -> Self {
        let boxed = Box::new(InnerFrame {
            // the reference count starts here at 1 since this is the first pointer to this new
            // data
            rc: atomic::AtomicUsize::new(1),
            buf: vec![0; size as usize],
            lock: Mutex::new(()),
        });

        Self {
            // `.unwrap()` is okay here since the pointer returned by `Box::into_raw` is guaranteed
            // not to be null
            ptr: NonNull::new(Box::into_raw(boxed)).unwrap(),
            phantom: PhantomData,
        }
    }

    fn count(this: &Self) -> usize {
        this.inner().rc.load(Ordering::Acquire)
    }

    fn inner(&self) -> &InnerFrame {
        unsafe { self.ptr.as_ref() }
    }

    fn copy_from_memory(
        &mut self,
        view: MemoryView,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let inner = unsafe { self.ptr.as_mut() };
        let _guard = inner.lock.lock()?;
        view.read(0, inner.buf.as_mut_slice())?;
        Ok(())
    }
}

// Frame is Send because access to mutable state is enforced internally with an atomic reference
// count.
unsafe impl Send for Frame {}
// Frame is Sync because we ensure nothing stored in a &Frame can be written to while that same
// thing could be read or written to from another &Frame -- enforced using atomic reference count.
unsafe impl Sync for Frame {}

impl Deref for Frame {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        let inner = unsafe { self.ptr.as_ref() };
        &inner.buf.as_slice()
    }
}

impl Clone for Frame {
    fn clone(&self) -> Self {
        let inner = unsafe { self.ptr.as_ref() };

        // relaxed ordering is okay here since we don't need to modify or access the inner data and
        // therefore don't need atomic synchronization
        let old_rc = inner.rc.fetch_add(1, Ordering::Relaxed);

        if old_rc >= isize::MAX as usize {
            std::process::abort();
        }

        Self {
            ptr: self.ptr,
            phantom: PhantomData,
        }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        let inner = unsafe { self.ptr.as_ref() };
        if inner.rc.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }
        atomic::fence(Ordering::Acquire);
        unsafe { Box::from_raw(self.ptr.as_ptr()) };
    }
}

impl AsRef<[u8]> for Frame {
    fn as_ref(&self) -> &[u8] {
        &self
    }
}

#[derive(Debug)]
struct InnerFrame {
    lock: Mutex<()>,
    rc: atomic::AtomicUsize,
    buf: Vec<u8>,
}

impl WasmDemoRunner {
    fn new() -> Self {
        let mut f = File::open("demo.wast").expect("opening wasm file");
        let mut wasm_module = String::new();
        f.read_to_string(&mut wasm_module)
            .expect("reading wasm module from file");

        let mut store = Store::default();
        let module = Module::new(&store, &wasm_module).expect("initializing wasm module");
        let import_object = imports! {};
        let instance = Instance::new(&mut store, &module, &import_object)
            .expect("initializing module instance");
        let memory = instance
            .exports
            .get_memory("image_buffer")
            .expect("retrieving image buffer");
        let view = memory.view(&store);

        let width: usize = 256;
        let height: usize = 256;
        let bytes_required = width as u64 * height as u64 * 4;

        if view.data_size() < bytes_required {
            let pages_required = bytes_required / wasmer::WASM_PAGE_SIZE as u64 + 1;
            memory
                .grow(&mut store, pages_required as u32)
                .expect("growing image buffer memory");
        }

        let runner = Self {
            wasm_store: store,
            module_instance: instance,
            width: width as u32,
            height: height as u32,
            bytes_required,
            frame_manager: FrameManager::new(bytes_required as usize),
            state: State::Running,
        };

        runner
    }

    fn run(&mut self) {
    }

    // fn title(&self) -> String {
    //     String::from("WebAssembly Demo Runner")
    // }

    // fn view(&self) -> Element<Self::Message> {
    //     let center: Element<Self::Message> = match &self.frame_manager.last_updated {
    //         Some(frame) => {
    //             let image_handle =
    //                 image::Handle::from_pixels(self.width, self.height, frame.clone());
    //             image::Viewer::new(image_handle)
    //                 .width(self.width as f32)
    //                 .height(self.height as f32)
    //                 .into()
    //         }
    //         None => text("missing frame!").into(),
    //     };
    //     let c = column![text("hello"), center, text("meow")];
    //     container(c).center_x().center_y().into()
    // }

    // fn subscription(&self) -> Subscription<Self::Message> {
    //     match self.state {
    //         State::Running => time::every(Duration::from_millis(10)).map(Message::Tick),
    //         State::Idle => Subscription::none(),
    //     }
    // }
}

impl WasmDemoRunner {
    fn tick(&mut self) -> std::result::Result<(), Box<dyn std::error::Error>> {
        self.frame_manager.last_updated = None;
        let tick = self
            .module_instance
            .exports
            .get_function("tick")
            .expect("retrieving 'tick' function instance from module");

        let _ = tick
            .call(&mut self.wasm_store, vec![].as_slice())
            .expect("calling 'tick' function instance from module");

        let mut frame = self.frame_manager.get_free_frame()?;
        let view = self
            .module_instance
            .exports
            .get_memory("image_buffer")?
            .view(&self.wasm_store);
        frame.copy_from_memory(view)?;
        self.frame_manager.last_updated = Some(frame.clone());
        Ok(())
    }
}

fn main() {
    let window = WindowDesc::new(make_ui()).title("wasm demo runner");

    let launcher = AppLauncher::with_window(window);

    let event_sink =  launcher.get_external_handle();

    let mut wasm_runner = WasmDemoRunner::new();

    thread::spawn(move || wasm_runner.run());

    launcher
        .log_to_console()
        .launch(Color::Rgba32(0xff0000))
        .expect("launch failed");
}

fn make_ui() -> impl Widget<Color> {
    Painter::new(|ctx, data, _env| {
        let rect = ctx.size().to_rounded_rect(5.0);
        ctx.fill(rect, data);
    })
    .fix_width(300.0)
    .fix_height(300.0)
    .padding(10.0)
    .center()
}
