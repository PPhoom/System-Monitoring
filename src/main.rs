use iced::executor;
use iced::widget::{button, column, container, row, text, Space};
// เพิ่ม Border เข้ามาใน use statement
use iced::{alignment, Alignment, Application, Border, Command, Element, Length, Settings, Theme, Color};
use std::time::Duration;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting System Utilities Application");
    App::run(Settings::default())
}

struct App {
    system_data: SystemData,
    is_refreshing: bool,
    last_error: Option<String>,
}

#[derive(Debug, Clone)]
struct SystemData {
    cpu_usage: f32,
    memory_used: f64,
    memory_total: f64,
    process_count: usize,
}

impl SystemData {
    fn mock() -> Self {
        SystemData {
            cpu_usage: 42.5,
            memory_used: 7.5,
            memory_total: 16.0,
            process_count: 4,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Refresh,
    DataRefreshed(SystemData),
    DataFetchFailed(String),
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Self {
                system_data: SystemData::mock(),
                is_refreshing: false,
                last_error: None,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("System Monitor")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Refresh => {
                if !self.is_refreshing {
                    self.is_refreshing = true;
                    self.last_error = None;
                    tracing::info!("Starting background refresh task");

                    return Command::perform(fetch_system_data_async(), |result| {
                        match result {
                            Ok(data) => Message::DataRefreshed(data),
                            Err(e) => Message::DataFetchFailed(e),
                        }
                    });
                }
                Command::none()
            }
            Message::DataRefreshed(new_data) => {
                self.system_data = new_data;
                self.is_refreshing = false;
                tracing::info!("Data refresh completed successfully");
                Command::none()
            }
            Message::DataFetchFailed(error_message) => {
                self.last_error = Some(error_message.clone());
                self.is_refreshing = false;
                tracing::error!("Error during data fetch: {}", error_message);
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let header = row![
            text("System Monitor").size(32),
            Space::with_width(Length::Fill),
            if self.is_refreshing {
                button(text("⏳ Refreshing...")).padding(10)
            } else {
                button(text("🔄 Refresh"))
                    .on_press(Message::Refresh)
                    .padding(10)
            },
        ]
        .spacing(20)
        .align_items(Alignment::Center);

        let cpu_value = format!("{:.1}%", self.system_data.cpu_usage);
        let memory_value = format!("{:.1} / {:.1} GB", self.system_data.memory_used, self.system_data.memory_total);
        let process_value = format!("{} running", self.system_data.process_count);

        let data_cards = row![
            create_card("CPU Usage", cpu_value),
            create_card("Memory", memory_value),
            create_card("Processes", process_value),
        ]
        .spacing(20);

        let status_message = if let Some(error) = &self.last_error {
            text(format!("❌ Error: {}", error)).style(Color::from_rgb(0.9, 0.3, 0.3))
        } else if self.is_refreshing {
            text("⏳ Refreshing data...")
        } else {
            text("✅ Ready").style(Color::from_rgb(0.3, 0.9, 0.3))
        };

        let content = column![
            header,
            Space::with_height(20),
            text("System Overview").size(24),
            Space::with_height(10),
            data_cards,
            Space::with_height(20),
            container(status_message).padding(10),
        ]
        .spacing(10)
        .padding(40);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::Dark
    }
}

async fn fetch_system_data_async() -> Result<SystemData, String> {
    tracing::info!("Background task: Fetching system data...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    use rand::Rng;
    let mut rng = rand::thread_rng();

    if rng.gen_bool(0.1) {
        tracing::error!("Background task: Simulated fetch failure.");
        Err("Failed to connect to the system service.".to_string())
    } else {
        let new_data = SystemData {
            cpu_usage: rng.gen_range(10.0..90.0),
            memory_used: rng.gen_range(4.0..12.0),
            memory_total: 16.0,
            process_count: rng.gen_range(50..200),
        };
        tracing::info!("Background task: Successfully fetched new data.");
        Ok(new_data)
    }
}

// =============================================================
// CREATE_CARD FUNCTION - แก้ไขที่นี่เป็นจุดสุดท้าย
// =============================================================
fn create_card(title: &str, value: String) -> Element<'static, Message> {
    let content = column![
        text(title).size(18),
        Space::with_height(10),
        text(value).size(36),
    ]
    .spacing(5)
    .padding(20)
    .align_items(Alignment::Center);

    container(content)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            // <<<< ****** จุดที่แก้ไขอยู่ตรงนี้ ****** >>>>
            container::Appearance {
                background: Some(palette.background.weak.color.into()),
                // เราสร้าง Border struct แล้วกำหนดให้กับ field `border`
                border: Border {
                    color: palette.background.strong.color,
                    width: 2.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .center_x()
        .into()
}