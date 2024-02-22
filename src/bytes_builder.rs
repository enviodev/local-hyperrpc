use bytes::Bytes;

pub struct BytesBuilder {
    list: Vec<Bytes>,
    total_len: usize,
}

impl BytesBuilder {
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
            total_len: 0,
        }
    }

    pub fn into_iter(self) -> std::vec::IntoIter<Bytes> {
        self.list.into_iter()
    }

    pub fn push(&mut self, b: Bytes) {
        self.total_len += b.len();
        self.list.push(b);
    }

    pub fn push_static(&mut self, s: &'static str) {
        self.push(Bytes::from_static(s.as_bytes()))
    }

    pub fn push_json_list<I: Iterator<Item = Bytes>>(&mut self, iter: I) {
        self.push_static("[");
        let mut start = "";
        for b in iter {
            self.push_static(start);
            self.push(b);
            start = ",";
        }
        self.push_static("]");
    }

    pub fn extend<I: IntoIterator<Item = Bytes>>(&mut self, iter: I) {
        for b in iter {
            self.push(b);
        }
    }

    // pub fn pop(&mut self) -> Option<Bytes> {
    //     let b = self.list.pop()?;
    //     self.total_len -= b.len();
    //     Some(b)
    // }

    pub fn total_len(&self) -> usize {
        self.total_len
    }

    // pub fn len(&self) -> usize {
    //     self.list.len()
    // }

    #[cfg(test)]
    pub fn build(&self) -> Bytes {
        use bytes::BytesMut;

        let mut dst = BytesMut::with_capacity(self.total_len);

        for b in self.list.iter() {
            dst.extend_from_slice(b);
        }

        dst.freeze()
    }
}
