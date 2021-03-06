use asynchron::{Futurize, ITaskHandle, Progress, SyncState};
use fltk::{app::*, button::*, frame::*, prelude::WidgetExt, window::*, *};
use reqwest::{Client, Response};
use std::time::Duration;
use tokio::runtime::Builder;

async fn fetch(url: &str, time_out: Duration) -> reqwest::Result<Response> {
    let client = Client::builder().timeout(time_out).build()?;
    let req = client.get(url).build()?;
    client.execute(req).await
}

fn main() -> std::io::Result<()> {
    let mut app = App::default();
    app.set_scheme(Scheme::Gtk);
    let mut wind = Window::default().with_size(400, 300);
    wind.set_label("Hello from rust");
    let mut timer_frame = Frame::default().with_label("");
    timer_frame.set_pos(0, 0);
    timer_frame.set_size(400, 100);

    let mut text_frame =
        Frame::default().with_label("Try hit Fetch button and let's see what happens...");
    text_frame.set_pos(100, 60);
    text_frame.set_size(200, 200);

    let mut button_fetch = Button::default().with_label("Fetch");
    button_fetch.set_pos(120, 210);
    button_fetch.set_size(80, 40);
    let mut button_cancel = Button::default()
        .with_label("Cancel")
        .right_of(&button_fetch, 10);
    button_cancel.set_size(80, 40);

    wind.show_with_args(&["-nokbd"]);

    let url = SyncState::new("https://www.rust-lang.org");
    let _url = url.clone();

    let rt = Builder::new_multi_thread().enable_all().build()?;
    // Clone the runtime handle, so the rt still reusable for the other tasks if needed.
    let rt_handle = rt.handle().clone();

    let reqwest = Futurize::task(
        0,
        move |this: ITaskHandle<String>| -> Progress<String, String> {
            rt_handle.block_on(async {
                let url = match _url.load() {
                    Some(url) => url,
                    _ => return Progress::Error("Unable to load URL, probably empty.".into()),
                };

                // Timeout connection for 5 seconds, so there's a noise if something goes wrong.
                let time_out = Duration::from_secs(5);

                let response = match fetch(url, time_out).await {
                    Ok(response) => response,
                    Err(e) => return Progress::Error(e.to_string().into()),
                };

                for i in 0..5 {
                    this.send(format!("checking status... {}", i));
                    std::thread::sleep(Duration::from_millis(100))
                }

                if !response.status().is_success() {
                    return Progress::Error(response.status().to_string().into());
                }

                let status = response.status().to_string();
                for _ in 0..5 {
                    // check if the task is canceled.
                    if this.should_cancel() {
                        return Progress::Canceled;
                    }
                    this.send(status.clone());
                    std::thread::sleep(Duration::from_millis(100))
                }

                match response.text().await {
                    Ok(text) => {
                        // and check here also.
                        if this.should_cancel() {
                            return Progress::Canceled;
                        }
                        Progress::Completed(text[0..100].to_string())
                    }
                    Err(e) => return Progress::Error(e.to_string().into()),
                }
            })
        },
    );

    let reqwest_fetch = reqwest.handle();
    let reqwest_cancel = reqwest.rt_handle();

    button_fetch.set_callback(move |_| reqwest_fetch.try_do());

    button_cancel.set_callback(move |_| {
        if reqwest_cancel.is_canceled() {
            println!("canceled")
        } else {
            reqwest_cancel.cancel()
        }
    });

    let mut label = String::new();

    let mut timer = 0;

    while app.wait() {
        reqwest.try_resolve(|progress, done| {
            match progress {
                Progress::Current(task_receiver) => {
                    button_fetch.set_label("Fetching...");
                    if let Some(value) = task_receiver {
                        text_frame.set_label(&value)
                    }
                }
                Progress::Canceled => label = "Request canceled.".to_owned(),
                Progress::Completed(value) => label = value,
                Progress::Error(e) => {
                    eprintln!("{}", &e);
                    label = e.into()
                }
            }

            if done {
                text_frame.set_label(&label);
                button_fetch.set_label("Fetch")
            }
        });

        if url.is_empty() {
            let value = if timer % 2 == 0 {
                "https://hyper.rs"
            } else {
                "https://www.rust-lang.org"
            };
            url.store(value);
            println!("url restored.");
        }

        timer += 1;

        timer_frame.set_label(timer.to_string().as_ref());
        wind.redraw();
        app::sleep(0.011);
        app::awake();
    }
    Ok(())
}
