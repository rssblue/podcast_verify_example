use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::{
    extract::Path,
    headers::ContentType,
    http::StatusCode,
    routing::{get, post},
    Router, TypedHeader,
};
use html_to_string_macro::html;
use rsa::pkcs8::LineEnding;
use rsa::{pkcs8::EncodePublicKey, RsaPrivateKey, RsaPublicKey};
use std::net::SocketAddr;

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
      verifyUrl=\"http://localhost:8081/verify/{}\"
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
    Html(html! {
        <h1>"Hosting Company"</h1>
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
    })
}
