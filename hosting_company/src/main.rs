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

mod view;

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
pub struct Podcast {
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
        .route("/feed/:slug/verify", post(verify))
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
    view::root(state.podcasts)
}

async fn verify(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    params: Query<VerifyParams>,
) -> (StatusCode, Html<String>) {
    let params: VerifyParams = params.0;

    let return_url = match params.return_url {
        Some(return_url) => return_url,
        None => {
            return view::verify(view::VerifyState::Error {
                podcast: None,
                return_url: None,
                message: html! { "URL parameter " <code>"returnUrl"</code> " is required." },
                code: StatusCode::BAD_REQUEST,
            })
        }
    };
    let return_url = match Url::parse(&return_url) {
        Ok(url) => url,
        Err(_) => {
            return view::verify(view::VerifyState::Error {
                podcast: None,
                return_url: None,
                message: html! { "Invalid " <code>"returnUrl"</code> "." },
                code: StatusCode::BAD_REQUEST,
            })
        }
    };
    let domain_name = match return_url.host_str() {
        Some(domain_name) => domain_name,
        None => {
            return view::verify(view::VerifyState::Error {
                podcast: None,
                return_url: None,
                message: html! { "Invalid " <code>"returnUrl"</code> "." },
                code: StatusCode::BAD_REQUEST,
            })
        }
    };
    let domain_name = match return_url.port() {
        Some(port) => format!("{}:{}", domain_name, port),
        None => domain_name.to_string(),
    };

    let podcast = match slug_to_podcast(state.podcasts.clone(), &slug) {
        Some(podcast) => podcast,
        None => {
            return view::verify(view::VerifyState::Error {
                podcast: None,
                return_url: Some(return_url),
                message: html! { "Podcast with slug " <code>{&slug}</code> " not found." },
                code: StatusCode::NOT_FOUND,
            })
        }
    };

    let encrypted_string = match params.encrypted_string {
        Some(encrypted_string) => encrypted_string,
        None => {
            return view::verify(view::VerifyState::Error {
                podcast: Some(podcast),
                return_url: Some(return_url),
                message: html! { "URL parameter " <code>"encryptedString"</code> " is required." },
                code: StatusCode::BAD_REQUEST,
            })
        }
    };

    view::verify(view::VerifyState::Neutral {
        podcasts: state.podcasts.clone(),
        podcast,
        return_url_scheme: return_url.scheme().to_string(),
        return_url_domain: domain_name,
    })
}
