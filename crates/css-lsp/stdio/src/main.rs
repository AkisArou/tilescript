use tilescript_css_lsp_core::{Session, protocol};
use lsp_server::{Connection, Message};

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let (connection, io_threads) = Connection::stdio();
    let server_capabilities = serde_json::to_value(protocol::server_capabilities())
        .expect("failed to serialize server capabilities");
    let _initialize_params =
        connection.initialize(server_capabilities).expect("failed to initialize lsp connection");

    let mut session = Session::new();

    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request).expect("shutdown handling failed") {
                    break;
                }

                let (response, events) = protocol::handle_request(&session, request);
                connection
                    .sender
                    .send(Message::Response(response))
                    .expect("failed to send response");

                for event in events {
                    connection.sender.send(event.message).expect("failed to send event");
                }
            }
            Message::Notification(notification) => {
                let events = protocol::handle_notification(&mut session, notification);
                for event in events {
                    connection.sender.send(event.message).expect("failed to send event");
                }
            }
            Message::Response(_) => {}
        }
    }

    io_threads.join().expect("failed to join lsp io threads");
}
