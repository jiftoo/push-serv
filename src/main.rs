use std::collections::HashMap;

use axum::{
	body::Bytes,
	extract::{DefaultBodyLimit, Query},
	http::{
		header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE},
		HeaderMap, StatusCode,
	},
	response::{Html, IntoResponse, Redirect},
	routing::{get, post},
	Router,
};
use include_dir::{include_dir, Dir};
use rand::{distributions::Alphanumeric, seq::IteratorRandom, Rng};

static IMAGES: Dir<'_> = include_dir!("src/images");

#[tokio::main]
async fn main() {
	let usage = format!("usage: {} <port> <max size in kibibytes>", std::env::args().next().unwrap());
	let port = std::env::args().nth(1).and_then(|x| x.parse::<u16>().ok()).unwrap_or_else(|| {
		eprintln!("port argument missing or invalid\n{usage}");
		std::process::exit(1);
	});
	let max_size_kib = std::env::args().nth(2).and_then(|x| x.parse::<usize>().ok()).unwrap_or_else(|| {
		eprintln!("max size argument missing or invalid\n{usage}");
		std::process::exit(1);
	});

	println!("Listening on port {}", port);
	println!("Max upload size: {} KiB", max_size_kib);

	let router = Router::new()
		.route("/", get(|| async { Redirect::permanent("/upload") }))
		.route("/upload", get(upload_page))
		.route("/image", get(image))
		.route("/upload", post(upload))
		.route("/max-size", get(move || async move { max_size_kib.to_string() }))
		.layer(DefaultBodyLimit::max(max_size_kib * 1024));

	let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

	axum::Server::bind(&addr).serve(router.into_make_service()).await.unwrap();
}

async fn upload_page() -> impl IntoResponse {
	// Html(include_str!("index.html"))
	Html(std::fs::read_to_string("./src/index.html").unwrap())
}

async fn upload(query: Query<HashMap<String, String>>, data: Bytes) -> Result<(), (StatusCode, &'static str)> {
	let filename = query.get("filename").ok_or((StatusCode::BAD_REQUEST, "filename missing"))?.to_owned();
	if !filename.chars().all(|x| x.is_alphabetic() || x.is_ascii_digit() || x == '_' || x == '.') {
		return Err((StatusCode::BAD_REQUEST, "filename contains invalid characters"));
	}
	let mut random_prefix = rand::thread_rng()
		.sample_iter(&Alphanumeric)
		.take(6)
		.map(char::from)
		.chain(std::iter::once('_'))
		.collect::<String>();
	random_prefix.push_str(&filename);
	let filename = random_prefix;
	tokio::fs::write(&filename, &data)
		.await
		.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to write file"))?;
	println!("downloaded file: filename: {filename}; size: {} bytes", data.len());
	Ok(())
}

async fn image() -> Result<(HeaderMap, &'static [u8]), StatusCode> {
	let image_file = IMAGES.files().choose(&mut rand::thread_rng()).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
	let mut headers = HeaderMap::new();
	headers.insert(CONTENT_DISPOSITION, "inline".parse().unwrap());
	headers.insert(
		CONTENT_TYPE,
		mime_guess::from_path(image_file.path())
			.first()
			.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
			.to_string()
			.parse()
			.unwrap(),
	);
	headers.insert(CACHE_CONTROL, "no-cache, no-store, must-revalidate".parse().unwrap());

	Ok((headers, image_file.contents()))
}
