use iced::{
    executor,
    Application, Command, Element, Settings, Subscription, Theme,
};

#[derive(Default)]
struct WasmDemoRunner {
    buf: Vec<u8>,
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
        (
            Self::default(),
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
        "meow".into()
    }
}

fn main() -> iced::Result {
    WasmDemoRunner::run(Settings::default())
}
