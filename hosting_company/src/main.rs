use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::{
    extract::Path,
    extract::Query,
    headers::ContentType,
    http::StatusCode,
    routing::{get, post},
    Router, TypedHeader,
};
use html_to_string_macro::html;
use rsa::pkcs8::LineEnding;
use rsa::{pkcs8::EncodePublicKey, RsaPrivateKey, RsaPublicKey};
use serde::Deserialize;
use std::net::SocketAddr;
use url::Url;

#[derive(Deserialize, Debug)]
struct VerifyParams {
    #[serde(default, rename = "encryptedString")]
    encrypted_string: Option<String>,
    #[serde(default, rename = "returnUrl")]
    return_url: Option<String>,
}

#[derive(Clone)]
struct Customer {
    email: String,
    // In practise, we would NEVER store unhashed passwords!
    password: String,
}

#[derive(Clone)]
struct Podcast {
    title: String,
    slug: String,
    owner: Customer,
}

impl Podcast {
    fn feed(&self, public_key: RsaPublicKey) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<rss version=\"2.0\" xmlns:podcast=\"https://podcastindex.org/namespace/1.0\">
  <channel>
    <title>{}</title>
    <podcast:verify
      verifyUrl=\"http://localhost:8081/feed/{}/verify\"
      publicKey=\"{}\"
      />
  </channel>
</rss>",
            self.title,
            self.slug,
            pem_to_base64(public_key.to_public_key_pem(LineEnding::LF).unwrap()),
        )
    }
}

/// Removes the header and footer from a PEM-encoded key, as well as any line breaks.
fn pem_to_base64(pem_string: String) -> String {
    pem_string
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("")
}

#[derive(Clone)]
struct AppState {
    podcasts: Vec<Podcast>,
    public_key: RsaPublicKey,
    private_key: RsaPrivateKey,
}

#[tokio::main]
async fn main() {
    let mut rng = rand::thread_rng();
    let bits = 2048;
    let private_key = RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
    let public_key = RsaPublicKey::from(&private_key);

    let customer_alice = Customer {
        email: String::from("alice@example.com"),
        password: String::from("password123"),
    };
    let customer_bob = Customer {
        email: String::from("bob@example.com"),
        password: String::from("password456"),
    };

    let podcasts = vec![
        Podcast {
            title: String::from("Alice's Podcast"),
            slug: String::from("alice-podcast"),
            owner: customer_alice,
        },
        Podcast {
            title: String::from("Bob's Podcast"),
            slug: String::from("bob-podcast"),
            owner: customer_bob,
        },
    ];

    let router = Router::new()
        .route("/", get(root))
        .route("/feed/:slug", get(feed))
        .route("/feed/:slug/verify", get(verify))
        .with_state(AppState {
            podcasts,
            public_key,
            private_key,
        });

    let port = 8081;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("Hosting Company listening on http://localhost:{port}");
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap();
}

async fn feed(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<(TypedHeader<ContentType>, impl IntoResponse), StatusCode> {
    let podcast = slug_to_podcast(state.podcasts, &slug).ok_or(StatusCode::NOT_FOUND)?;

    Ok((
        TypedHeader(ContentType::xml()),
        podcast.feed(state.public_key),
    ))
}

fn slug_to_podcast(podcasts: Vec<Podcast>, slug: &str) -> Option<Podcast> {
    podcasts.into_iter().find(|podcast| podcast.slug == slug)
}

async fn root(State(state): State<AppState>) -> impl IntoResponse {
    let title = "Hosting Company";
    base_html(
        title,
        html! {
            <h1>{title}</h1>
            <p>"Podcasts we host:"</p>
            <ul>
            {
                let mut my_html = vec![];
                for podcast in state.podcasts {
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

async fn verify(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    params: Query<VerifyParams>,
) -> impl IntoResponse {
    let podcast = match slug_to_podcast(state.podcasts, &slug) {
        Some(podcast) => podcast,
        None => {
            return (
                StatusCode::NOT_FOUND,
                base_html(
                    "Not Found",
                    html! {
                        <h1>"Not Found"</h1>
                        <p>"No podcast with slug " <code>{slug}</code> " found."</p>
                    },
                ),
            )
        }
    };
    let title = format!("Verify ownership of “{}”", podcast.title);

    let params: VerifyParams = params.0;

    let encrypted_string = match params.encrypted_string {
        Some(encrypted_string) => encrypted_string,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                base_html(
                    &title,
                    html! {
                        <h1>{title.clone()}</h1>
                        {error(html!{ "URL parameter " <code>"encryptedString"</code> " is required." })}
                    },
                ),
            )
        }
    };

    let return_url = match params.return_url {
        Some(return_url) => return_url,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                base_html(
                    &title,
                    html! {
                        <h1>{title.clone()}</h1>
                        {error(html!{ "URL parameter " <code>"returnUrl"</code> " is required." })}
                    },
                ),
            )
        }
    };

    let return_url = match Url::parse(&return_url) {
        Ok(url) => url,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                base_html(
                    &title,
                    html! {
                        <h1>{title.clone()}</h1>
                        {error(html!{ "Invalid " <code>"returnUrl"</code> "." })}
                    },
                ),
            )
        }
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
                    <input type="email" id="email" name="email" autocomplete="off"/>
                    <label for="password">"Password"</label>
                    <input type="password" id="password" name="password" autocomplete="off"/>

                    <button type="submit">"Verify"</button>
                </form>
            },
        ),
    )
}

fn base_html(title: &str, main: String) -> Html<String> {
    Html(html! {
        <!DOCTYPE html>
        <html>
            <head>
                <meta charset="UTF-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1.0" />
                <link rel="stylesheet" href="https://unpkg.com/mvp.css" />

                <title>{title}</title>
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
