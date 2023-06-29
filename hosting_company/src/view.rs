use crate::Podcast;
use axum::http::StatusCode;
use axum::response::Html;
use html_to_string_macro::html;
use url::Url;

pub fn root(podcasts: Vec<Podcast>) -> Html<String> {
    let title = "Hosting Company";
    base_html(
        title,
        html! {
            <h1>{title}</h1>
            <p>"Podcasts we host:"</p>
            <ul>
            {
                let mut my_html = vec![];
                for podcast in podcasts {
                    my_html.push(html! {
                        <li>
                            <a
                                href=format!("/feed/{}", podcast.slug)
                                rel="noreferrer"
                                target="_blank"
                                >
                                {podcast.title}
                            </a>
                        </li>
                    });
                }
                my_html.join("")
            }
            </ul>
        },
    )
}

pub enum VerifyState {
    Neutral {
        podcasts: Vec<Podcast>,
        podcast: Podcast,
        return_url_scheme: String,
        return_url_domain: String,
    },
    Error {
        podcast: Option<Podcast>,
        return_url: Option<Url>,
        message: String,
        code: StatusCode,
    },
}

pub fn verify(state: VerifyState) -> (StatusCode, Html<String>) {
    match state {
        VerifyState::Neutral {
            podcasts,
            podcast,
            return_url_scheme,
            return_url_domain,
        } => {
            let title = html! {
                "Log in to verify ownership of “" {podcast.title} "” to " <a href={format!("{return_url_scheme}://{return_url_domain}")} rel="noreferrer" target="_blank">{return_url_domain}</a>
            };

            (
                StatusCode::OK,
                base_html(
                    &title,
                    html! {
                        <h1>{&title}</h1>
                        <form method="POST" autocomplete="off">
                            <input autocomplete="false" name="hidden" type="text" style="display:none;" />

                            <label for="email">"Email"</label>
                            <input type="email" list="email-list" id="email" name="email" autocomplete="off"/>
                            <datalist id="email-list">
                            {
                                podcasts.iter().map(|podcast| {
                                    html! {
                                        <option value={podcast.owner.email.to_string()} />
                                    }
                                }).collect::<Vec<_>>().join("")
                            }
                            </datalist>

                            <label for="password">"Password ("<a href="https://github.com/rssblue/podcast_verify_example#login" rel="noreferrer" target="_blank">"hint"</a>")"</label>
                            <input type="password" id="password" name="password" autocomplete="off"/>

                            <button type="submit">"Log in"</button>
                        </form>
                    },
                ),
            )
        }
        VerifyState::Error {
            podcast,
            return_url,
            message,
            code,
        } => {
            let title = match podcast {
                Some(podcast) => format!("Verify ownership of “{}”", podcast.title),
                None => "Verify ownership".to_string(),
            };

            let error_title = format!("Error: {}", StatusCode::to_string(&code));
            (
                code,
                base_html(
                    &error_title,
                    html! {
                        <h1>{&title}</h1>
                        {error(message)}
                        {
                            match return_url {
                                Some(return_url) => html! {
                                    <strong>"Redirecting to "<a href={return_url.to_string()} rel="noreferrer">{return_url.to_string()}</a>" in "<span id="countdown">"10"</span>" seconds..."</strong>
                                    <script>
                                        "let seconds = 10;"
                                        "let countdown = document.getElementById('countdown');"
                                        "let interval = setInterval(() => {"
                                            "seconds -= 1;"
                                            "countdown.innerText = seconds;"
                                            "if (seconds <= 0) {"
                                                "clearInterval(interval);"
                                                "window.location.href = '" {return_url.to_string()} "';"
                                            "}"
                                        "}, 1000);"
                                    </script>
                                },
                                None => html! {}
                            }
                        }
                    },
                ),
            )
        }
    }
}

fn base_html(title: &str, main: String) -> Html<String> {
    Html(html! {
        <!DOCTYPE html>
        <html>
            <head>
                <meta charset="UTF-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1.0" />
                <link rel="stylesheet" href="https://unpkg.com/mvp.css" />

                <title>{dissolve::strip_html_tags(title).join("")}</title>
            </head>
            <body>
                <header>
                    <nav>
                        <span>"🔵 Hosting Company"</span>
                        <ul>
                            <li><a href="/">"Home"</a></li>
                        </ul>
                    </nav>
                </header>
                <main>
                    {main}
                </main>
            </body>
        </html>
    })
}

fn error(message: String) -> String {
    html! {
        <h2 style="color: crimson;">"Error"</h2>
        <p>{message}</p>
    }
}
