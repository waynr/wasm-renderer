use std::fs::File;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};

use iced::widget::{column, container, image, text};
use iced::{executor, Application, Command, Element, Settings, Theme};
use wasmer::{imports, Instance, MemoryView, Module, Store};

struct WasmDemoRunner {
    wasm_store: Store,
    module_instance: Instance,

    width: u32,
    height: u32,
    bytes_required: u64,

    frame: Arc<Frame>,
    frame_manager: FrameManager,
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
            frames: vec![Frame::new(size); 10],
        }
    }

    fn get_free_frame(&mut self) -> std::result::Result<Frame, Box<dyn std::error::Error>> {
        let frame = self
            .frames
            .iter_mut()
            .find(|f| Arc::strong_count(&f.0) == 1)
            .ok_or("couldn't find free frame")?;

        Ok(frame.clone())
    }
}

#[derive(Clone, Debug)]
struct Frame(Arc<Mutex<InnerFrame>>);

impl Frame {
    fn new(size: usize) -> Self {
        Self(Arc::new(Mutex::new(InnerFrame::new(size))))
    }

    fn copy_from_memory(
        self,
        view: MemoryView,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut binding = match self.0.lock() {
            Ok(inner) => inner,
            Err(_e) => return Err("whoops".into()),
        };
        let mut s = binding.buf.as_mut_slice();
        view.read(0, &mut s)?;
        Ok(())
    }
}

#[derive(Debug)]
struct InnerFrame {
    buf: Vec<u8>,
}

impl InnerFrame {
    fn new(size: usize) -> Self {
        Self {
            buf: vec![0; size as usize],
        }
    }
}

impl AsRef<[u8]> for Frame {
    fn as_ref(&self) -> &[u8] {
        match self.0.lock() {
            Ok(f) => {
                f.buf.as_slice()
            },
            Err(_) => {
                panic!()
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Tick,
}

impl Application for WasmDemoRunner {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
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

        let mut runner = Self {
            wasm_store: store,
            module_instance: instance,
            width: width as u32,
            height: height as u32,
            bytes_required,
            frame: Arc::new(Frame::new(bytes_required as usize)),
            frame_manager: FrameManager::new(bytes_required as usize),
        };
        runner.tick();
        (runner, Command::none())
    }

    fn title(&self) -> String {
        String::from("WebAssembly Demo Runner")
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Tick => {
                self.tick();
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Self::Message> {
        let center: Element<Self::Message> = match &self.frame_manager.last_updated {
            Some(frame) => {
                let image_handle =
                    image::Handle::from_pixels(self.width, self.height, frame.clone());
                image::Viewer::new(image_handle)
                    .width(self.width as f32)
                    .height(self.height as f32)
                    .into()
            }
            None => text("missing frame!").into(),
        };
        let c = column![text("hello"), center, text("meow")];
        container(c).center_x().center_y().into()
    }
}

impl WasmDemoRunner {
    fn tick(&mut self) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let tick = self
            .module_instance
            .exports
            .get_function("tick")
            .expect("retrieving 'tick' function instance from module");

        let _ = tick
            .call(&mut self.wasm_store, vec![].as_slice())
            .expect("calling 'tick' function instance from module");

        let frame = self.frame_manager.get_free_frame()?;
        let view = self
            .module_instance
            .exports
            .get_memory("image_buffer")?
            .view(&self.wasm_store);
        frame.copy_from_memory(view)?;
        Ok(())
    }
}

fn main() -> iced::Result {
    WasmDemoRunner::run(Settings::default())
}
