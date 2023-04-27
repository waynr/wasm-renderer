use std::fs::File;
use std::io::prelude::*;

use iced::widget::{column, container, image, text};
use iced::{executor, Application, Command, Element, Settings, Theme};
use wasmer::{imports, Instance, Module, Store};

struct WasmDemoRunner {
    wasm_store: Store,
    module_instance: Instance,
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
        let tick = instance
            .exports
            .get_function("tick")
            .expect("retrieving 'tick' function instance from module");
        let _ = tick
            .call(&mut store, &[])
            .expect("calling 'tick' function instance from module");
        (
            Self {
                wasm_store: store,
                module_instance: instance,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("WebAssembly Demo Runner")
    }

    fn update(&mut self, _message: Self::Message) -> Command<Self::Message> {
        Command::none()
    }

    fn view(&self) -> Element<Self::Message> {
        match self.get_image() {
            Ok(bs) => {
                let image_handle = image::Handle::from_pixels(128, 128, bs);
                let img = image::Viewer::new(image_handle)
                    .width(128)
                    .height(128);
                let content = column![text("hello"), img, text("meow"),];
                container(content).center_x().center_y().into()
            }
            Err(e) => text(format!("failed to retrieve wasm-generated image: {0}", e)).into(),
        }
    }
}

impl WasmDemoRunner {
    fn get_image(&self) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
        let buf: &mut [u8; 256 * 256] = &mut [0; 256 * 256];
        let view = self
            .module_instance
            .exports
            .get_memory("image_buffer")?
            .view(&self.wasm_store);
        view.read(0, buf)?;
        Ok(buf.to_vec())
    }
}

fn main() -> iced::Result {
    WasmDemoRunner::run(Settings::default())
}
