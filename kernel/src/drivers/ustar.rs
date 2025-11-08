const BLOCK_SIZE: usize = 512;
const USTAR_SIGNATURE_OFFSET: usize = 257;
const FILE_SIZE_OFFSET: usize = 0x7c;
const FILE_SIZE_LENGTH: usize = 11;
const HEADER_SIZE: usize = 512;

pub struct USTarFile {
    filename: &'static str,
    data: &'static [u8],
    size: usize,
}

impl USTarFile {
    pub fn new(filename: &'static str, data: &'static [u8], size: usize) -> Self {
        return Self {
            filename,
            data,
            size,
        };
    }

    pub fn read(&self, offset: usize, bytes: usize) -> &'static [u8] {
        return &self.data[offset..offset + bytes];
    }

    pub fn read_to_end(&self, offset: usize) -> &'static [u8] {
        return &self.data[offset..self.get_size() - 1];
    }

    pub fn read_all(&self) -> &'static [u8] {
        return &self.data;
    }

    pub fn get_size(&self) -> usize {
        return self.size;
    }

    pub fn get_name(&self) -> &'static str {
        return self.filename;
    }
}

fn oct2bin(octal_bytes: &[u8]) -> usize {
    let mut result = 0;

    for &byte in octal_bytes {
        if byte.is_ascii_digit() {
            result = result * 8 + (byte - b'0') as usize;
        } else {
            break;
        }
    }
    return result;
}

pub struct USTarFileIterator {
    data: &'static [u8],
    ptr: usize,
}

impl Iterator for USTarFileIterator {
    type Item = USTarFile;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ptr >= self.data.len() {
            return None;
        }

        if self.ptr + HEADER_SIZE > self.data.len() {
            return None;
        }

        if self.ptr + USTAR_SIGNATURE_OFFSET + 5 > self.data.len() {
            return None;
        }

        let signature =
            &self.data[self.ptr + USTAR_SIGNATURE_OFFSET..self.ptr + USTAR_SIGNATURE_OFFSET + 5];

        if signature != b"ustar" {
            return None;
        }

        let size_slice = self
            .data
            .get(self.ptr + FILE_SIZE_OFFSET..self.ptr + FILE_SIZE_OFFSET + FILE_SIZE_LENGTH)?;
        let filesize = oct2bin(size_slice);

        let name_field = &self.data[self.ptr..self.ptr + 100];
        let name_len = name_field.iter().position(|&b| b == 0).unwrap_or(100);
        let name_slice = &name_field[..name_len];

        let data_start = self.ptr + HEADER_SIZE;
        let data_end = data_start + filesize;

        if data_end > self.data.len() {
            return None;
        }

        let file_data = &self.data[data_start..data_end];
        let name = match str::from_utf8(name_slice) {
            Ok(n) => n,
            Err(_) => {
                self.ptr = self.data.len();
                return None;
            }
        };

        let blocks_needed = if filesize == 0 {
            0
        } else {
            (filesize + BLOCK_SIZE - 1) / BLOCK_SIZE
        };

        let next_ptr = match self.ptr.checked_add((blocks_needed + 1) * BLOCK_SIZE) {
            Some(p) => p,
            None => {
                self.ptr = self.data.len();
                return Some(USTarFile::new(name, file_data, filesize));
            }
        };

        self.ptr = next_ptr;

        return Some(USTarFile::new(name, file_data, filesize));
    }
}

pub struct USTar {
    data: &'static [u8],
}

impl USTar {
    pub fn new(data: &'static [u8]) -> Self {
        return Self { data };
    }

    pub fn read_file(&self, filename: &[u8]) -> Option<USTarFile> {
        let mut ptr = 0;

        while ptr + HEADER_SIZE <= self.data.len() {
            let signature =
                &self.data[ptr + USTAR_SIGNATURE_OFFSET..ptr + USTAR_SIGNATURE_OFFSET + 5];
            if signature != b"ustar" {
                break;
            }

            let size_slice = self
                .data
                .get(ptr + FILE_SIZE_OFFSET..ptr + FILE_SIZE_OFFSET + FILE_SIZE_LENGTH)?;
            let filesize = oct2bin(size_slice);

            let name_field = &self.data[ptr..ptr + 100];
            let name_len = name_field.iter().position(|&b| b == 0).unwrap_or(100);
            let header_name = &name_field[..name_len];

            if header_name == filename {
                let data_start = ptr + HEADER_SIZE;
                let data_end = data_start + filesize;

                if data_end <= self.data.len() {
                    let file_data = &self.data[data_start..data_end];

                    let name =
                        str::from_utf8(header_name).expect("(USTAR) Unable to read filename!");
                    return Some(USTarFile::new(name, file_data, filesize));
                }
            }

            let blocks_needed = if filesize == 0 {
                0
            } else {
                (filesize + BLOCK_SIZE - 1) / BLOCK_SIZE
            };

            let next_ptr = ptr.checked_add((blocks_needed + 1) * BLOCK_SIZE)?;

            if next_ptr > self.data.len() {
                break;
            }

            ptr = next_ptr;
        }

        return None;
    }

    pub fn files(&self) -> USTarFileIterator {
        return USTarFileIterator {
            data: self.data,
            ptr: 0,
        };
    }
}
