use hyper::Client;
use std::{convert::Infallible, net::SocketAddr};

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CSRxInfo<'a> {
    #[serde(rename(deserialize = "gatewayID"))]
    gateway_id: &'a str,
    time: Option<&'a str>,
    #[serde(rename(deserialize = "timeSinceGPSEpoch"))]
    time_since_gps_epoch: &'a str,
    rssi: i32,
    #[serde(rename(deserialize = "loRaSNR"))]
    lora_snr: f64,
    channel: u32,
    rf_chain: u32,
    board: u32,
    antenna: u32,
    crc_status: &'a str,
}

#[derive(Debug, Deserialize)]
struct CSTags<'a> {
    #[serde(rename(deserialize = "ThingsBoardAccessToken"))]
    things_board_access_token: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CSUp<'a> {
    #[serde(rename(deserialize = "applicationID"))]
    application_id: &'a str,
    application_name: &'a str,
    device_name: &'a str,
    #[serde(rename(deserialize = "devEUI"))]
    dev_eui: &'a str,
    rx_info: Vec<CSRxInfo<'a>>,
    adr: bool,
    dr: u32,
    f_cnt: u32,
    f_port: u32,
    data: &'a str,
    tags: CSTags<'a>,
    confirmed_uplink: bool,
    dev_addr: &'a str,
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 9999));

    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(uplink)) });

    let server = Server::bind(&addr).serve(make_svc);

    let graceful = server.with_graceful_shutdown(shutdown_signal());

    if let Err(e) = graceful.await {
        eprintln!("server error: {}", e);
    }
}

async fn uplink(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let mut response = Response::new(Body::empty());

    match (req.method(), req.uri().query()) {
        (&Method::POST, Some("event=up")) => {
            let full_body = hyper::body::to_bytes(req.into_body()).await?;

            let a: CSUp = match serde_json::from_slice(&full_body) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Invalid up JSON object! {}", e);

                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(response);
                }
            };

            println!("Sending uplink from {} to ThingsBoard", a.device_name);
            let client = Client::new();

            // Send attributes
            let req = Request::builder()
                .method(Method::POST)
                .uri(format!(
                    "http://oats2.ecn.purdue.edu:10000/api/v1/{}/attributes",
                    a.tags.things_board_access_token
                ))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{
                    "application_name": "{}",
                    "application_id": "{}",
                    "device_name": "{}",
                    "dev_eui": "{}"
                }}"#,
                    a.application_name, a.application_id, a.device_name, a.dev_eui
                )))
                .unwrap();

            let resp = client.request(req).await.unwrap();

            println!("Response: {}", resp.status());

            // Send telemetry
            let req = Request::builder()
                .method(Method::POST)
                .uri(format!(
                    "http://oats2.ecn.purdue.edu:10000/api/v1/{}/telemetry",
                    a.tags.things_board_access_token
                ))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{
                    "data_data": "{}",
                    "fport": {},
                    "fcnt": {},
                    "dr": {},
                    "rssi": {},
                    "snr": {}
                }}"#,
                    a.data,
                    a.f_port,
                    a.f_cnt,
                    a.dr,
                    a.rx_info.get(0).unwrap().rssi,
                    a.rx_info.get(0).unwrap().lora_snr
                )))
                .unwrap();

            let resp = client.request(req).await.unwrap();

            println!("Response: {}", resp.status());
        }
        _ => {
            println!("Received an {}. Skipping.", req.uri().query().unwrap());
            *response.status_mut() = StatusCode::NOT_FOUND;
        }
    };

    Ok(response)
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
}
