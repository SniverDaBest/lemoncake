use alloc::format;
use alloc::string::{FromUtf8Error, String, ToString};
use alloc::vec::Vec;

// Base64 alphabet used for encoding
const BASE64_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

// Function to get the decode table for Base64 decoding
fn get_decode_table() -> [u8; 128] {
    let mut table = [255; 128];
    let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (i, &byte) in chars.iter().enumerate() {
        table[byte as usize] = i as u8;
    }
    table
}

/// Encode function for Base64
pub fn encode_bytes(input: &[u8]) -> [u8; 128] {
    let mut output = [0; 128];
    let mut output_index = 0;

    let mut i = 0;
    while i < input.len() {
        let byte1 = input[i];
        let byte2 = if i + 1 < input.len() { input[i + 1] } else { 0 };
        let byte3 = if i + 2 < input.len() { input[i + 2] } else { 0 };

        let index1 = (byte1 >> 2) & 0x3F;
        let index2 = ((byte1 & 0x03) << 4) | (byte2 >> 4);
        let index3 = ((byte2 & 0x0F) << 2) | (byte3 >> 6);
        let index4 = byte3 & 0x3F;

        if output_index < 128 {
            output[output_index] = BASE64_ALPHABET[index1 as usize];
            output_index += 1;
        }
        if output_index < 128 {
            output[output_index] = BASE64_ALPHABET[index2 as usize];
            output_index += 1;
        }

        if i + 1 < input.len() {
            if output_index < 128 {
                output[output_index] = BASE64_ALPHABET[index3 as usize];
                output_index += 1;
            }
        } else {
            if output_index < 128 {
                output[output_index] = b'=';
                output_index += 1;
            }
        }

        if i + 2 < input.len() {
            if output_index < 128 {
                output[output_index] = BASE64_ALPHABET[index4 as usize];
                output_index += 1;
            }
        } else {
            if output_index < 128 {
                output[output_index] = b'=';
                output_index += 1;
            }
        }

        i += 3;
    }

    output
}

/// Decode function for Base64
pub fn decode_bytes(input: &[u8]) -> Result<Vec<u8>, ()> {
    let mut output = Vec::new();
    let decode_table = get_decode_table();

    let mut i = 0;
    while i < input.len() {
        let byte1 = input[i];
        let byte2 = if i + 1 < input.len() {
            input[i + 1]
        } else {
            b'='
        };
        let byte3 = if i + 2 < input.len() {
            input[i + 2]
        } else {
            b'='
        };
        let byte4 = if i + 3 < input.len() {
            input[i + 3]
        } else {
            b'='
        };

        let index1 = decode_table[byte1 as usize];
        let index2 = decode_table[byte2 as usize];
        let index3 = decode_table[byte3 as usize];
        let index4 = decode_table[byte4 as usize];

        if index1 == 255
            || index2 == 255
            || (byte3 != b'=' && index3 == 255)
            || (byte4 != b'=' && index4 == 255)
        {
            return Err(());
        }

        let byte = (index1 << 2) | (index2 >> 4);
        output.push(byte);

        if byte3 != b'=' {
            let byte = ((index2 & 0x0F) << 4) | (index3 >> 2);
            output.push(byte);
        }

        if byte4 != b'=' {
            let byte = ((index3 & 0x03) << 6) | index4;
            output.push(byte);
        }

        i += 4;
    }

    Ok(output)
}

/// Convert the encoded byte array to a String
pub fn encoded_to_string(encoded: [u8; 128]) -> Result<String, FromUtf8Error> {
    // Find the length of the encoded data by checking for '=' characters
    let len = encoded
        .iter()
        .position(|&x| x == b'=')
        .unwrap_or(encoded.len());

    // Create a byte slice of the valid encoded data
    let encoded_bytes = &encoded[..len];

    // Convert the byte slice to a String
    String::from_utf8(encoded_bytes.to_vec())
}

/// Convert the decoded byte vector to a String
pub fn decoded_to_string(decoded: Vec<u8>) -> Result<String, FromUtf8Error> {
    String::from_utf8(decoded)
}

pub fn encode(bytes: &[u8]) -> String {
    let encoded_input = encode_bytes(bytes);
    match encoded_to_string(encoded_input) {
        Ok(result) => return result,
        Err(e) => return format!("An error occured when trying to encode! {}", e.to_string()),
    }
}

pub fn decode(bytes: &[u8]) -> String {
    match decode_bytes(bytes) {
        Ok(decoded_input) => match decoded_to_string(decoded_input) {
            Ok(result) => return result,
            Err(e) => return format!("An error occured when trying to decode! {}", e.to_string()),
        },
        Err(e) => return format!("An error occured when trying to decode! {:?}", e).to_string(),
    }
}
