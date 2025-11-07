use iced::executor;
// Import Container and Renderer
use iced::widget::{
    column, container, row, text, Button, Radio, Scrollable, Space, Container,
};
use iced::{
    alignment, Alignment, Application, Border, Color, Command, Element, Length, 
    Renderer, // Keep Renderer import
    Settings, Subscription, Theme,
};
use std::time::Duration;
use sysinfo::{Pid, System};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::ProjectDirs;

// =============================================================
// CONFIGURATION (No changes)
// =============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ThemeChoice {
    Light,
    Dark,
}

impl ThemeChoice {
    fn to_theme(&self) -> Theme {
        match self {
            ThemeChoice::Light => Theme::Light,
            ThemeChoice::Dark => Theme::Dark,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppSettings {
    theme: ThemeChoice,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { theme: ThemeChoice::Dark }
    }
}

impl AppSettings {
    fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "YourOrg", "SystemMonitor").map(|dirs| {
            let path = dirs.config_dir().join("settings.json");
            tracing::info!("Config path: {:?}", path);
            path
        })
    }

    async fn load() -> Result<Self, String> {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                let content = tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| e.to_string())?;
                serde_json::from_str(&content).map_err(|e| e.to_string())
            } else {
                Ok(Self::default())
            }
        } else {
            Err("Could not find config directory".to_string())
        }
    }

    async fn save(self) -> Result<(), String> {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
            let content = serde_json::to_string_pretty(&self).map_err(|e| e.to_string())?;
            tokio::fs::write(path, content)
                .await
                .map_err(|e| e.to_string())
        } else {
            Err("Could not find config directory".to_string())
        }
    }
}

// =============================================================
// APPLICATION (No changes)
// =============================================================

pub fn main() -> iced::Result {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting System Utilities Application");
    App::run(Settings::default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    Processes,
    Settings,
}

#[derive(Debug, Clone)]
struct ProcessData { 
    pid: Pid, 
    name: String, 
    cpu_usage: f32, 
    memory: u64 
}

#[derive(Debug, Clone)]
struct SystemData { 
    cpu_usage: f32, 
    memory_used: f64, 
    memory_total: f64, 
    process_count: usize 
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotificationLevel { Success, Error }

#[derive(Debug, Clone)]
struct StatusMessage { 
    message: String, 
    level: NotificationLevel 
}

impl StatusMessage {
    fn success(message: &str) -> Self { 
        Self { 
            message: message.to_string(), 
            level: NotificationLevel::Success 
        } 
    }
    fn error(message: &str) -> Self { 
        Self { 
            message: message.to_string(), 
            level: NotificationLevel::Error 
        } 
    }
}

struct App {
    system: System,
    active_tab: Tab,
    dashboard_data: SystemData,
    process_list: Vec<ProcessData>,
    selected_process: Option<Pid>,
    show_kill_confirm: Option<Pid>,
    last_status_message: Option<StatusMessage>,
    settings: AppSettings,
    is_loading: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    SettingsLoaded(Result<AppSettings, String>),
    SettingsSaved(Result<(), String>),
    ThemeChanged(ThemeChoice),
    TabSelected(Tab),
    ProcessSelected(Pid),
    KillProcessRequested(Pid),
    KillProcessConfirmed(Pid),
    KillProcessCancelled,
    ClearStatusMessage,
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let mut sys = System::new_all();
        sys.refresh_all();

        let to_gb = |bytes: u64| bytes as f64 / (1024.0 * 1024.0 * 1024.0);

        let dashboard_data = SystemData {
            cpu_usage: sys.global_cpu_info().cpu_usage(),
            memory_used: to_gb(sys.used_memory()),
            memory_total: to_gb(sys.total_memory()),
            process_count: sys.processes().len(),
        };

        let process_list = App::build_process_list(&sys);

        (
            Self {
                system: sys,
                active_tab: Tab::Dashboard,
                dashboard_data,
                process_list,
                selected_process: None,
                show_kill_confirm: None,
                last_status_message: None,
                settings: AppSettings::default(),
                is_loading: true,
            },
            Command::perform(AppSettings::load(), Message::SettingsLoaded),
        )
    }

    fn title(&self) -> String {
        String::from("System Monitor")
    }

    fn theme(&self) -> Theme {
        self.settings.theme.to_theme()
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SettingsLoaded(Ok(settings)) => {
                self.settings = settings;
                self.is_loading = false;
                tracing::info!("Settings loaded successfully");
                Command::none()
            }
            Message::SettingsLoaded(Err(e)) => {
                self.is_loading = false;
                tracing::error!("Failed to load settings: {}", e);
                self.last_status_message = Some(StatusMessage::error("Failed to load settings"));
                Command::none()
            }
            Message::ThemeChanged(theme_choice) => {
                self.settings.theme = theme_choice;
                tracing::info!("Theme changed, saving settings...");
                Command::perform(self.settings.clone().save(), Message::SettingsSaved)
            }
            Message::SettingsSaved(Ok(())) => {
                tracing::info!("Settings saved successfully.");
                self.last_status_message = Some(StatusMessage::success("Settings saved ✅"));
                Command::perform(tokio::time::sleep(Duration::from_secs(3)), |_| Message::ClearStatusMessage)
            }
            Message::SettingsSaved(Err(e)) => {
                tracing::error!("Failed to save settings: {}", e);
                self.last_status_message = Some(StatusMessage::error("Failed to save settings ⚠️"));
                Command::perform(tokio::time::sleep(Duration::from_secs(3)), |_| Message::ClearStatusMessage)
            }
            
            Message::Tick => {
                self.system.refresh_all(); 
                let to_gb = |bytes: u64| bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                self.dashboard_data = SystemData {
                    cpu_usage: self.system.global_cpu_info().cpu_usage(),
                    memory_used: to_gb(self.system.used_memory()),
                    memory_total: to_gb(self.system.total_memory()),
                    process_count: self.system.processes().len(),
                };
                self.process_list = App::build_process_list(&self.system);
                if let Some(pid) = self.selected_process {
                    if !self.system.processes().contains_key(&pid) {
                        self.selected_process = None;
                    }
                }
                Command::none()
            }
            Message::TabSelected(tab) => {
                self.active_tab = tab;
                Command::none()
            }
            Message::ProcessSelected(pid) => {
                self.selected_process = Some(pid);
                Command::none()
            }
            Message::KillProcessRequested(pid) => {
                self.show_kill_confirm = Some(pid);
                Command::none()
            }
            Message::KillProcessCancelled => {
                self.show_kill_confirm = None;
                Command::none()
            }
            Message::KillProcessConfirmed(pid) => {
                self.show_kill_confirm = None; 
                let (status_message, command) = if let Some(process) = self.system.process(pid) {
                    if process.kill() {
                        let msg = StatusMessage::success(&format!("Process {} killed successfully ✅", pid));
                        let cmd = Command::perform(tokio::time::sleep(Duration::from_secs(3)), |_| Message::ClearStatusMessage);
                        (msg, cmd)
                    } else {
                        let err_msg = format!("Failed to kill process {} ⚠️ (Permission denied?)", pid);
                        let msg = StatusMessage::error(&err_msg);
                        let cmd = Command::perform(tokio::time::sleep(Duration::from_secs(3)), |_| Message::ClearStatusMessage);
                        (msg, cmd)
                    }
                } else {
                    let err_msg = format!("Tried to kill non-existent process {}", pid);
                    (
                        StatusMessage::error(&err_msg),
                        Command::perform(tokio::time::sleep(Duration::from_secs(3)), |_| Message::ClearStatusMessage)
                    )
                };
                self.last_status_message = Some(status_message);
                command
            }
            Message::ClearStatusMessage => {
                self.last_status_message = None;
                Command::none()
            }
        }
    }

    // Keep explicit signature
    fn view(&self) -> Element<'_, Message, Theme, Renderer> {
        if self.is_loading {
            let content = text("Loading settings...")
                .size(32)
                .horizontal_alignment(alignment::Horizontal::Center);
            // FIX: Use explicit types with Container::new
            return Container::<Message, Theme, Renderer>::new(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .into();
        }

        let tabs = row![
            create_tab_button("Dashboard", Tab::Dashboard, self.active_tab),
            create_tab_button("Processes", Tab::Processes, self.active_tab),
            create_tab_button("Settings", Tab::Settings, self.active_tab),
        ]
        .spacing(10);

        let page_content = match self.active_tab {
            Tab::Dashboard => self.view_dashboard(),
            Tab::Processes => self.view_processes(),
            Tab::Settings => self.view_settings(),
        };
        
        let status_bar: Element<'_, Message, Theme, Renderer> = if let Some(status) = &self.last_status_message {
            let (bg_color, text_color) = match status.level {
                NotificationLevel::Success => (Color::from_rgb(0.2, 0.6, 0.2), Color::WHITE),
                NotificationLevel::Error => (Color::from_rgb(0.8, 0.2, 0.2), Color::WHITE),
            };
            // FIX: Use explicit types with Container::new
            Container::<Message, Theme, Renderer>::new(
                text(status.message.clone())
                    .style(iced::theme::Text::Color(text_color))
                    .horizontal_alignment(alignment::Horizontal::Center)
            )
            .width(Length::Fill)
            .padding(10)
            .style(move |theme: &Theme| container::Appearance {
                background: Some(iced::Background::Color(bg_color)),
                border: Border { radius: 5.0.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        } else {
            // FIX: Use explicit types with Container::new
            Container::<Message, Theme, Renderer>::new(Space::with_height(0.0))
                .padding(10)
                .into()
        };

        let main_content = column![
            tabs,
            Space::with_height(20),
            page_content,
            Space::with_height(10),
            status_bar
        ]
        .spacing(10)
        .padding(40)
        .align_items(Alignment::Center);

        // Show modal if needed
        if let Some(pid_to_kill) = self.show_kill_confirm {
            let process_name = self.system.process(pid_to_kill)
                                        .map_or("Unknown Process", |p| p.name());
            
            // FIX: Use explicit types with Container::new
            Container::<Message, Theme, Renderer>::new(
                column![
                    // Main content (dimmed)
                    // FIX: Use explicit types with Container::new
                    Container::<Message, Theme, Renderer>::new(main_content)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(|theme: &Theme| container::Appearance {
                            background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.3))),
                            ..Default::default()
                        }),
                    // Modal dialog centered
                    // FIX: Use explicit types with Container::new
                    Container::<Message, Theme, Renderer>::new(
                        column![
                            text(format!("Kill Process: {} (PID: {})?", process_name, pid_to_kill)).size(24),
                            Space::with_height(10),
                            text("Are you sure? This action cannot be undone."),
                            Space::with_height(20),
                            row![
                                Button::new(text("Cancel"))
                                    .on_press(Message::KillProcessCancelled)
                                    .style(iced::theme::Button::Secondary)
                                    .padding(10),
                                Button::new(text("Yes, Kill Process"))
                                    .on_press(Message::KillProcessConfirmed(pid_to_kill))
                                    .style(iced::theme::Button::Destructive)
                                    .padding(10),
                            ].spacing(10).align_items(Alignment::Center),
                        ]
                        .spacing(10)
                        .padding(30)
                        .align_items(Alignment::Center)
                    )
                    .style(|theme: &Theme| {
                        let palette = theme.extended_palette();
                        container::Appearance {
                            background: Some(iced::Background::Color(palette.background.base.color)),
                            border: Border {
                                color: palette.background.strong.color,
                                width: 2.0,
                                radius: 10.0.into(),
                            },
                            ..Default::default()
                        }
                    })
                    .width(Length::Fixed(500.0))
                    .center_x(),
                ]
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
        } else {
            main_content.into()
        }
    }
}

impl App {
    fn build_process_list(sys: &System) -> Vec<ProcessData> {
        let mut processes: Vec<ProcessData> = sys
            .processes()
            .values()
            .map(|p| ProcessData {
                pid: p.pid(),
                name: p.name().to_string(),
                cpu_usage: p.cpu_usage(),
                memory: p.memory(),
            })
            .collect();
        processes.sort_by(|a, b| {
            b.cpu_usage
                .partial_cmp(&a.cpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        processes
    }

    // Keep explicit signature
    fn view_dashboard(&self) -> Element<'_, Message, Theme, Renderer> {
        let header = row![
            text("System Monitor").size(32),
            Space::with_width(Length::Fill),
            text("🟢 Real-time").style(Color::from_rgb(0.3, 0.9, 0.3)),
        ]
        .spacing(20)
        .align_items(Alignment::Center);

        let cpu_value = format!("{:.1}%", self.dashboard_data.cpu_usage);
        let memory_value = format!("{:.1} / {:.1} GB", self.dashboard_data.memory_used, self.dashboard_data.memory_total);
        let process_value = format!("{} running", self.dashboard_data.process_count);

        let data_cards = row![
            create_card("CPU Usage", cpu_value),
            create_card("Memory", memory_value),
            create_card("Processes", process_value),
        ]
        .spacing(20);

        column![
            header,
            Space::with_height(20),
            text("System Overview").size(24),
            Space::with_height(10),
            data_cards,
        ]
        .align_items(Alignment::Center)
        .into()
    }

    // Keep explicit signature
    fn view_processes(&self) -> Element<'_, Message, Theme, Renderer> {
        // Keep explicit internal signature
        let process_rows: Element<'_, Message, Theme, Renderer> = self.process_list.iter()
            .fold(column![
                row![
                    text("PID").width(Length::Fixed(100.0)),
                    text("Name").width(Length::Fill),
                    text("CPU %").width(Length::Fixed(100.0)),
                    text("Memory").width(Length::Fixed(100.0)),
                ].spacing(10).padding(5),
                // FIX: Use explicit types with Container::new
                Container::<Message, Theme, Renderer>::new(Space::with_height(2.0))
                    .style(iced::theme::Container::Box)
                    .width(Length::Fill)
            ].spacing(5), 
            |col, process| {
                let pid = process.pid;
                let mem_mb = process.memory as f64 / (1024.0 * 1024.0);
                let process_row = row![
                    text(pid.to_string()).width(Length::Fixed(100.0)),
                    text(process.name.clone()).width(Length::Fill),
                    text(format!("{:.1}", process.cpu_usage)).width(Length::Fixed(100.0)),
                    text(format!("{:.1} MB", mem_mb)).width(Length::Fixed(100.0)),
                ]
                .spacing(10)
                .align_items(Alignment::Center)
                .padding(5);
                
                col.push(
                    Button::new(process_row)
                        .on_press(Message::ProcessSelected(pid))
                        .style(if self.selected_process == Some(pid) {
                            iced::theme::Button::Primary
                        } else {
                            iced::theme::Button::Text
                        })
                )
            })
            .into();

        let process_table = Scrollable::new(process_rows)
            .width(Length::FillPortion(2))
            .height(Length::Fixed(600.0));

        // Keep explicit internal signature
        let detail_pane: Element<'_, Message, Theme, Renderer> = if let Some(pid) = self.selected_process {
            if let Some(process) = self.system.process(pid) {
                let mem_mb = process.memory() as f64 / (1024.0 * 1024.0);
                column![
                    text(format!("Details for: {}", process.name())).size(24),
                    Space::with_height(10),
                    text(format!("PID: {}", process.pid())),
                    text(format!("CPU: {:.1} %", process.cpu_usage())),
                    text(format!("Memory: {:.1} MB", mem_mb)),
                    text(format!("Status: {:?}", process.status())),
                    text(format!("Executable: {}", process.exe().map_or("N/A", |p| p.to_str().unwrap_or("N/A")))),
                    text(format!("Command: {}", process.cmd().join(" "))),
                    Space::with_height(Length::Fill),
                    Button::new(text("Kill Process").style(Color::WHITE))
                        .on_press(Message::KillProcessRequested(pid))
                        .style(iced::theme::Button::Destructive)
                        .padding(10)
                ]
                .spacing(10)
                .padding(20)
                .width(Length::Fill)
                .into()
            } else {
                // FIX: Use explicit types with Container::new
                Container::<Message, Theme, Renderer>::new(text("Process disappeared."))
                    .width(Length::Fill)
                    .align_x(alignment::Horizontal::Center)
                    .center_y()
                    .into()
            }
        } else {
            // FIX: Use explicit types with Container::new
            Container::<Message, Theme, Renderer>::new(text("Select a process from the list"))
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center)
                .center_y()
                .into()
        };

        // FIX: Use explicit types with Container::new
        let detail_container = Container::<Message, Theme, Renderer>::new(detail_pane)
            .width(Length::FillPortion(1))
            .height(Length::Fixed(600.0))
            .style(iced::theme::Container::Box);

        row![
            process_table,
            detail_container,
        ]
        .spacing(20)
        .width(Length::Fixed(1200.0))
        .into()
    }

    // Keep explicit signature
    fn view_settings(&self) -> Element<'_, Message, Theme, Renderer> {
        let light_radio = Radio::new(
            "Light Theme",
            ThemeChoice::Light,
            Some(self.settings.theme),
            Message::ThemeChanged,
        );
        
        let dark_radio = Radio::new(
            "Dark Theme",
            ThemeChoice::Dark,
            Some(self.settings.theme),
            Message::ThemeChanged,
        );

        // FIX: Use explicit types with Container::new
        Container::<Message, Theme, Renderer>::new(
            column![
                text("Application Settings").size(24),
                Space::with_height(20),
                light_radio,
                dark_radio,
            ]
            .spacing(10)
            .padding(20)
        )
        .width(Length::Fixed(1200.0))
        .height(Length::Fixed(600.0))
        .align_x(alignment::Horizontal::Left)
        .style(iced::theme::Container::Box)
        .into()
    }
}

// Keep explicit signature
// Note: We need 'static lifetime here
fn create_card(title: &str, value: String) -> Element<'static, Message, Theme, Renderer> {
    let content = column![
        text(title).size(18),
        Space::with_height(10),
        text(value).size(36),
    ]
    .spacing(5)
    .padding(20)
    .align_items(Alignment::Center);

    // FIX: Use explicit types with Container::new
    Container::<'static, Message, Theme, Renderer>::new(content)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Appearance {
                background: Some(iced::Background::Color(palette.background.weak.color)),
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

// Keep explicit signature
// Note: We need 'static lifetime here
fn create_tab_button(label: &str, tab: Tab, active_tab: Tab) -> Element<'static, Message, Theme, Renderer> {
    let is_active = tab == active_tab;
    Button::new(
        text(label)
            .size(20)
            .horizontal_alignment(alignment::Horizontal::Center)
    )
    .on_press(Message::TabSelected(tab))
    .style(if is_active {
        iced::theme::Button::Primary
    } else {
        iced::theme::Button::Secondary
    })
    .padding(10)
    .width(Length::Fixed(150.0))
    .into()
}

#[cfg(test)]
mod tests {
    use super::System;
    #[test]
    fn test_sysinfo_data_retrieval() {
        let mut sys = System::new_all();
        assert!(sys.total_memory() > 0, "ควรอ่านค่า Memory รวมได้");
        sys.refresh_cpu();
        std::thread::sleep(std::time::Duration::from_millis(250));
        sys.refresh_cpu();
        let cpu_usage = sys.global_cpu_info().cpu_usage();
        assert!(cpu_usage >= 0.0 && cpu_usage <= 100.0, "CPU Usage ควรอยู่ระหว่าง 0-100");
        sys.refresh_processes();
        let process_count = sys.processes().len();
        assert!(process_count > 0, "ควรมี Process รันอยู่");
    }
}
