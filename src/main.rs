use axum::Router;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let app = Router::new().fallback_service(ServeDir::new("src/static"));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();

    println!("RPS - Web Server listening on: http://0.0.0.0:8000");

    axum::serve(listener, app).await.unwrap();
}
