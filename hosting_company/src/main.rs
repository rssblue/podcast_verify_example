use axum::response::{Html, IntoResponse};
use axum::{
    extract::Path,
    headers::ContentType,
    http::StatusCode,
    routing::{get, post},
    Router, TypedHeader,
};
use html_to_string_macro::html;
use std::net::SocketAddr;

struct Customer<'a> {
    email: &'a str,
    // In practise, we would NEVER store unhashed passwords!
    password: &'a str,
}

struct Podcast<'a> {
    title: &'a str,
    slug: &'a str,
    owner: Customer<'a>,
}

impl Podcast<'_> {
    fn feed(&self) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<rss version=\"2.0\" xmlns:podcast=\"https://podcastindex.org/namespace/1.0\">
  <channel>
    <title>{}</title>
    <podcast:verify
      verifyUrl=\"http://localhost:8081/verify/{}\"
      publicKey=\"\"
      />
  </channel>
</rss>",
            self.title, self.slug,
        )
    }
}

const CUSTOMER_ALICE: Customer = Customer {
    email: "alice@example.com",
    password: "password123",
};
const PODCAST_ALICE: Podcast = Podcast {
    title: "Alice Podcast",
    slug: "alice-podcast",
    owner: CUSTOMER_ALICE,
};

const CUSTOMER_BOB: Customer = Customer {
    email: "bob@example.com",
    password: "password456",
};
const PODCAST_BOB: Podcast = Podcast {
    title: "Bob Podcast",
    slug: "bob-podcast",
    owner: CUSTOMER_BOB,
};

static PODCASTS: &[&Podcast] = &[&PODCAST_ALICE, &PODCAST_BOB];

#[tokio::main]
async fn main() {
    let router = Router::new().route("/feed/:slug", get(feed));

    let port = 8081;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("Hosting Company listening on http://localhost:{port}");
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap();
}

async fn feed(
    Path(slug): Path<String>,
) -> Result<(TypedHeader<ContentType>, impl IntoResponse), StatusCode> {
    let podcast = PODCASTS
        .iter()
        .find(|podcast| podcast.slug == slug)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok((TypedHeader(ContentType::xml()), podcast.feed()))
}