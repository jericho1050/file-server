use super::request::HttpRequest;
use super::request::Version;
use std::fmt::Display;
use std::io;
use std::path::{Path, PathBuf};
use infer;
use mime_guess::from_path;
use url_escape::decode;
use log::{error, warn};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct HttpResponse {
    pub version: Version,
    pub status: ResponseStatus,
    pub content_length: usize,
    pub content_type: String,
    pub accept_ranges: AcceptRanges,
    pub response_body: Vec<u8>,
    pub current_path: String,
}

impl HttpResponse {
    pub fn new(request: &HttpRequest) -> io::Result<HttpResponse> {
        let version = Version::V2_0;
        let mut status: ResponseStatus = ResponseStatus::NotFound;
        let mut content_length: usize = 0;
        let mut content_type = "text/plain".to_string();
        let mut accept_ranges: AcceptRanges = AcceptRanges::None;
        let current_path = request.resource.path.clone();
        let mut response_body = Vec::new();

        let rootcwd = std::env::current_dir()?.canonicalize()?;
        let decoded_path = decode(&request.resource.path).into_owned();
        let resource_path = Path::new(&decoded_path);
        let resource = rootcwd.join(&resource_path).canonicalize()?;

        // Ensure the new path is within the server root directory
        if !resource.starts_with(&rootcwd) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Access denied"));
        }

        if resource.exists() {
            if resource.is_file() {
                let content = std::fs::read(&resource)?;
                content_length = content.len();
                status = ResponseStatus::OK;
                accept_ranges = AcceptRanges::Bytes;

                // Detect content type using infer
                if let Some(kind) = infer::get(&content) {
                    content_type = kind.mime_type().to_string();
                } else {
                    // Fallback to mime_guess
                    content_type = from_path(&resource).first_or_octet_stream().to_string();
                }

                response_body = content;
            } else if resource.is_dir() {
                // Handle directory listing or navigation
                let mut begin_html = r#"
<!DOCTYPE html> 
<html> 
<head> 
    <meta charset="utf-8"> 
    <style>
        body { font-family: Arial, sans-serif; }
        a { text-decoration: none; color: blue; }
        a:hover { text-decoration: underline; }
    </style>
</head> 
<body>"#.to_string();

                let header = format!("<h1>Currently in {}</h1>", resource.to_string_lossy());

                let mut dir_listing = String::new();
                if let Some(parent) = resource.parent() {
                    let parent_link = parent.strip_prefix(&rootcwd).unwrap_or(parent).to_str().unwrap_or("..");
                    dir_listing.push_str(&format!("<a href=\"{}\">..</a><br>", parent_link));
                }

                for entry in WalkDir::new(&resource).max_depth(1).min_depth(1) {
                    let entry = entry?;
                    let path = entry.path();
                    let display = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let link = path.strip_prefix(&rootcwd).map_err(|err| io::Error::new(io::ErrorKind::Other, err))?.to_str().unwrap_or(&display);
                    let link = format!("/{}", link.trim_start_matches('/')); // Ensure the link is correctly constructed
                    dir_listing.push_str(&format!("<a href=\"{}\">{}</a><br>", html_escape::encode_text(&link), html_escape::encode_text(&display)));
                }

                content_length = dir_listing.len();
                status = ResponseStatus::OK;
                content_type = "text/html".to_string();

                let end_html = r#"
</body>
</html>"#.to_string();

                let content = format!(
                    "{}{}{}{}",
                    begin_html, header, dir_listing, end_html
                );
                response_body = content.into_bytes();
            }
        } else {
            error!("Path does not exist: {:?}", resource);
            let four_o_four = "<html>\n<body>\n<h1>404 Not Found</h1>\n</body>\n</html>\n";
            content_length = four_o_four.len();
            let content = format!(
                "{} {}\r\n{}\r\nContent-Length: {}\r\nContent-Type: text/html\r\n\r\n{}",
                version, status, accept_ranges, content_length, four_o_four
            );
            response_body = content.into_bytes();
        }

        Ok(HttpResponse {
            version,
            status,
            content_length,
            content_type,
            accept_ranges,
            response_body,
            current_path,
        })
    }
}

#[derive(Debug)]
pub enum ResponseStatus {
    OK = 200,
    NotFound = 404,
}

impl Display for ResponseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            ResponseStatus::OK => "200 OK",
            ResponseStatus::NotFound => "404 NOT FOUND",
        };
        write!(f, "{}", msg)
    }
}

#[derive(Debug)]
enum AcceptRanges {
    Bytes,
    None,
}

impl Display for AcceptRanges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            AcceptRanges::Bytes => "accept-ranges: bytes",
            AcceptRanges::None => "accept-ranges: none",
        };
        write!(f, "{}", msg)
    }
}