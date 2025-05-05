use std::collections::HashMap;

use tokio::signal::unix::{signal, SignalKind};
use zbus::{zvariant::Value, Connection, Result};
use serde_json::json;
use std::io::Write;
use indexmap::IndexMap;


#[derive(Debug, Clone)]
struct Notification {
    content: String,
    read: bool,
}

struct NotificationServer {
    history: IndexMap<u32, Notification>,
    visible_on_bar: Option<usize>,
    last_notification_id: u32,
}

impl NotificationServer {
    fn new() -> Self {
        Self {
            history: IndexMap::new(),
            visible_on_bar: None,
            last_notification_id: 0,
        }
    }

    fn add_to_history(&mut self, id: u32, notification: Notification) {
        self.history.insert(id, notification);
    }

    fn get_ui_data(&self) -> (&str, String) {
        let notification_list: String = self
            .history
            .iter()
            .rev()
            .map(|notification| {
                if notification.1.read {
                    format!("<span size='xx-large'><b>•</b> {}</span>",&notification.1.content)
                }
                else {
                    format!("<span color='#00d69e' size='xx-large'><b>• {}</b></span>", notification.1.content)
                }
            }
            )
            .collect::<Vec<_>>()
            .join("\n");
        let text = match self.visible_on_bar {
            None => "",
            Some(index) => &self.history[index].content,
        };
        (text, notification_list)
    }

    fn display_notifications_on_bar(&self) {
        let (text, notification_list) = self.get_ui_data();
        let waybar_output = json!({
            "text": text,
            "tooltip": notification_list,
        });
        println!("{}", waybar_output);
    }
    
    fn new_notification_display(&self) {
        let (text, notification_list) = self.get_ui_data();
        let waybar_output = json!({
            "text": text,
            "tooltip": notification_list,
            "class": "notify"
        });
        println!("{}", waybar_output);
    }

    fn new_id(&mut self) -> u32 {
        if self.last_notification_id == u32::MAX {
            self.last_notification_id = 1;
        } else {
            self.last_notification_id += 1;
        }
        self.last_notification_id
    }

    fn previous_notification(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.visible_on_bar {
            Some(index) if index == 0 => {
                self.mark_read(index);
            }
            Some(index) => {
                self.mark_read(index);
                self.visible_on_bar = Some(index - 1);
            }
            None => {
                // Technically not possible at this point
                // But it makes sense not to display any notification (just display the icon) while keeping the history
                self.visible_on_bar = Some(0);
            }
        }

        self.display_notifications_on_bar();
    }

    fn next_notification(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.visible_on_bar {
            Some(index) if index == self.history.len() - 1 => {
                self.mark_read(index);
            }
            Some(index) => {
                self.mark_read(index);
                self.visible_on_bar = Some(index + 1);
            }
            None => {
                // Technically not possible at this point
                // But it makes sense not to display any notification (just display the icon) while keeping the history
                self.visible_on_bar = Some(0);
            }
        }

        self.display_notifications_on_bar();
    }
    
    fn mark_read(&mut self, index: usize) {
            self.history[index].read = true;
    }

    fn mark_read_and_render(&mut self) {
        if let Some(index) = self.visible_on_bar {
            self.mark_read(index);
        }
        self.display_notifications_on_bar();
    }
}


#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    fn notify(
        &mut self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, Value>,
        expire_timeout: i32,
    ) -> u32 {
        let notification = Notification {
            content: format!("[{}] <b>{}</b>: {}", app_name, summary, body),
            read: false,
        };
        let id = if replaces_id == 0 { self.new_id() } else { replaces_id};
        self.add_to_history(id, notification);
        self.visible_on_bar = Some(self.history.len() - 1);
        self.new_notification_display();

        id
    }

    fn get_capabilities(&self) -> Vec<&str> {
        vec!["body", "actions"]
    }

    fn close_notification(&mut self, id: u32) -> zbus::fdo::Result<()> {
        if id == 0 {
            // Well, that violates spec. Could use signals instead
            // id=0 shouldn't be used anyway according to the spec so it sounds reasonable
            self.history.shift_remove_index(self.visible_on_bar.unwrap());
        } else {
            self.history.shift_remove(&id);
        }
        if self.visible_on_bar >= Some(self.history.len()) {
            self.visible_on_bar = if self.history.len() == 0 { None } else {
                Some(self.history.len() - 1)
            };
        }
        self.display_notifications_on_bar();
        Ok(())
    }

    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        (
            "Glance",
            "Glance",
            env!("CARGO_PKG_VERSION"),
            "1.3",
        )
    }
}


#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    std::io::stdout().flush().unwrap();
    let connection = Connection::session().await?;
    let server = connection.object_server();
    server.at("/org/freedesktop/Notifications", NotificationServer::new()).await?;
    connection.request_name("org.freedesktop.Notifications").await?;

    let sigrtmin = unsafe { libc::SIGRTMIN() };
    let mut signal_mark_read = signal(SignalKind::from_raw(sigrtmin))?;
    let mut signal_previous = signal(SignalKind::from_raw(sigrtmin + 2))?;
    let mut signal_next = signal(SignalKind::from_raw(sigrtmin + 3))?;

    loop {
        tokio::select! {
            _ = signal_mark_read.recv() => {
                if let Ok(server) = server.interface::<_, NotificationServer>("/org/freedesktop/Notifications").await {
                    server.get_mut().await.mark_read_and_render();
                }
            },
            _ = signal_previous.recv() => {
                if let Ok(server) = server.interface::<_, NotificationServer>("/org/freedesktop/Notifications").await {
                    server.get_mut().await.previous_notification();
                }
            },
            _ = signal_next.recv() => {
                if let Ok(server) = server.interface::<_, NotificationServer>("/org/freedesktop/Notifications").await {
                    server.get_mut().await.next_notification();
                }
            },
        }
    }
}
