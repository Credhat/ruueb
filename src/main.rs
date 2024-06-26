use core::time;
use ruueb::operator;
use std::{
    env, fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};
fn handle_connection_new(mut stream: TcpStream) -> io::Result<()> {
    let mut buf_reader = BufReader::new(&mut stream);
    let request_line = buf_reader.by_ref().lines().nth(0).unwrap().unwrap();

    let mut html_path: PathBuf;
    let current_path = env::current_dir()?;
    html_path = PathBuf::from(current_path);
    html_path.push("assets/html/");

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let (method, path, _version) = (parts[0], parts[1], parts[2]);

    let (header, html_file) = match (method, path) {
        ("GET", "/") => ("HTTP/1.1 200 OK", "index.html"),
        ("GET", "/sleep") => {
            thread::sleep(Duration::from_secs(5));
            ("HTTP/1.1 200 OK", "index.html")
        }
        ("POST", "/add") => {
            let body = get_request_body(&mut buf_reader)?;
            operator::add_product(&body)?;
            ("HTTP/1.1 200 OK", "index.html")
        }
        ("DELETE", "/delete") => {
            let body = get_request_body(&mut buf_reader)?;
            operator::delete_product(&body)?;
            ("HTTP/1.1 200 OK", "index.html")
        }
        ("GET", "/products") => {
            let mut products_output = Vec::new();
            operator::read_products(&mut products_output, true)?;
            let products_content = String::from_utf8(products_output).unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                products_content.len(),
                products_content
            );
            stream.write_all(response.as_bytes()).unwrap();
            return Ok(());
        }
        _ => ("HTTP/1.1 404 NOT FOUND", "404.html"),
    };

    html_path.push(html_file);
    let content = fs::read_to_string(html_path)?;
    let len_content = content.len();
    let response = format!("{header}\r\nContent-Length: {len_content}\r\n\r\n{content}");
    stream.write_all(response.as_bytes()).unwrap();
    Ok(())
}

fn get_request_body(buf_reader: &mut BufReader<&mut TcpStream>) -> std::io::Result<String> {
    let mut body = String::new();
    let mut content_length = 0;
    for line in buf_reader.by_ref().lines() {
        let line = line?;
        if line.is_empty() {
            break;
        }
        if line.starts_with("Content-Length:") {
            content_length = line[16..].trim().parse().unwrap();
        }
    }
    buf_reader
        .take(content_length as u64)
        .read_to_string(&mut body)?;
    Ok(body)
}

fn handle_connection(mut stream: TcpStream) -> std::io::Result<()> {
    // handle the connection here
    let buf_reader = BufReader::new(&mut stream);
    let request_line = buf_reader.lines().nth(0).unwrap().unwrap();

    // let http_request: Vec<_> = buf_reader
    //     .lines()
    //     .map(|result| result.unwrap())
    //     .take_while(|l| !l.is_empty())
    //     .collect();
    let mut html_path: PathBuf;

    let current_path = env::current_dir()?;
    html_path = PathBuf::from(current_path);
    html_path.push("assets/html/");

    let (header, html_file) = match &request_line[..] {
        "GET / HTTP/1.1" => ("HTTP/1.1 200 OK", "index.html"),
        "GET /sleep HTTP/1.1" => {
            thread::sleep(time::Duration::from_secs(5));
            ("HTTP/1.1 200 OK", "index.html")
        }
        value => {
            if value.contains("GET /sleep") && value.contains("HTTP/1.1") {
                let mut parts = value.split_ascii_whitespace();
                let sleep_time: u64 = match parts.nth(1).unwrap().replace("/sleep", "").parse() {
                    Ok(time) => time,
                    Err(e) => {
                        eprintln!("Error on parsing sleep time: {:#?}", e);
                        5
                    }
                };
                thread::sleep(time::Duration::from_secs(sleep_time.min(60)));
                ("HTTP/1.1 200 OK", "index.html")
            } else {
                ("HTTP/1.1 404 NOT FOUND", "404.html")
            }
        }
    };

    html_path.push(html_file);
    let content = fs::read_to_string(html_path)?;
    let len_content = content.len();
    let response = format!("{header}\r\nContent-Length: {len_content}\r\n\r\n{content}");
    stream.write_all(response.as_bytes()).unwrap();
    // println!("Request: {:#?}", request_line);
    Ok(())
}

fn main() -> std::io::Result<()> {
    let base_server = ServerAddr::local_at(7089);

    let listener = TcpListener::bind(base_server.to_bind_string())?;
    let task_pool = ThreadPool::new(4);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                task_pool.execute(|| {
                    // handle_connection(stream).unwrap();
                    handle_connection_new(stream).unwrap();
                });
                // thread::spawn(|| {
                //     handle_connection(stream).unwrap();
                // });
            }
            Err(e) => {
                /* connection failed */
                println!("Connection failed: {}", e)
            }
        }
    }
    Ok(())
}

struct ServerAddr {
    ip: String,
    port: u32,
}

impl ServerAddr {
    fn new(ip: &str, port: u32) -> ServerAddr {
        ServerAddr {
            ip: String::from(ip),
            port,
        }
    }

    fn local_at(port: u32) -> ServerAddr {
        ServerAddr::new("127.0.0.1", port)
    }

    fn to_bind_string(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

struct ThreadPool {
    worker_num: usize,
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

// struct Job {}

impl ThreadPool {
    pub fn new(worker_num: usize) -> ThreadPool {
        assert!(worker_num > 0);
        let (sender, receiver) = mpsc::channel();

        let receiver: Arc<Mutex<mpsc::Receiver<Job>>> = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(worker_num);

        for id in 0..worker_num {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            worker_num,
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(task);
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
    // receiver: mpsc::Receiver<Job>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();
            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; let's executing.");
                    job();
                }
                Err(_) => {
                    println!("Worker {id} disconnected; Shutting down.");
                    break;
                }
            }
        });
        Worker {
            id,
            thread: Some(thread),
        }
    }
}
