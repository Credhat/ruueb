use std::fs;
use std::io::{self, Write};

const PRODUCT_CSV: &str = "./assets/products.csv";

/// Adds a product to the CSV file.
/// The `body` parameter should be a comma-separated string like "id,name,price,quantity".
pub fn add_product(body: &str) -> io::Result<()> {
    // Open the file in read mode to check if headers are present
    let file = fs::OpenOptions::new().read(true).open(PRODUCT_CSV)?;

    let mut rdr = csv::Reader::from_reader(file);

    // Check if the file is empty or if headers are absent
    let is_empty = rdr.records().count() == 0;
    let has_headers = !is_empty && rdr.headers().is_ok();

    // Re-open the file in append mode
    let file = fs::OpenOptions::new().append(true).open(PRODUCT_CSV)?;

    let mut wtr = csv::Writer::from_writer(file);

    let product: Vec<&str> = body.split(',').collect();
    if product.len() != 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid product data",
        ));
    }

    // If the file is empty or headers are absent, write headers
    if is_empty || !has_headers {
        wtr.write_record(&["id", "name", "price", "quantity"])?;
    }

    // Check if ID already exists in the file
    let mut found_id = false;
    let id_to_match = product[0];
    for record in rdr.records() {
        let record = record?;
        if record.iter().next() == Some(id_to_match) {
            // ID found, overwrite the existing record
            wtr.write_record(&product)?;
            found_id = true;
            break;
        }
    }

    // If ID not found, append a new record
    if !found_id {
        wtr.write_record(&product)?;
    }

    wtr.flush()?;
    Ok(())
}

/// Deletes a product from the CSV file by its name.
/// The `body` parameter should be a comma-separated string, and the second element is the product name.
pub fn delete_product(body: &str) -> io::Result<()> {
    let product_name = body.split(',').nth(1).unwrap(); // get product name
    let mut rdr = csv::Reader::from_path(PRODUCT_CSV)?;
    let mut records: Vec<csv::StringRecord> = rdr.records().filter_map(Result::ok).collect();

    let original_len = records.len();
    records.retain(|record| record.get(1) != Some(product_name));

    if records.len() == original_len {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Product not found"));
    }

    let mut wtr = csv::Writer::from_path(PRODUCT_CSV)?;
    // Write headers
    wtr.write_record(&["id", "name", "price", "quantity"])?;
    for record in records {
        wtr.write_record(&record)?;
    }
    wtr.flush()?;
    Ok(())
}

/// Reads products from the CSV file and writes them to the provided writer.
/// The `with_headers` parameter determines if the CSV output should include headers.
pub fn read_products(wtr: &mut dyn Write, with_headers: bool) -> io::Result<()> {
    let mut rdr = csv::Reader::from_path(PRODUCT_CSV)?;
    let mut writer = csv::WriterBuilder::new()
        .has_headers(with_headers)
        .from_writer(wtr);

    for result in rdr.records() {
        let record = result?;
        writer.write_record(&record)?;
    }
    writer.flush()?;
    Ok(())
}
