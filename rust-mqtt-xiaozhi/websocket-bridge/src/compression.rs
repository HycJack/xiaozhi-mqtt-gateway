use flate2::{Compress, Decompress, FlushCompress, FlushDecompress};
use bytes::{Bytes, BytesMut};
use log::{debug, info};
use anyhow::{Result, Context};

pub struct CompressionHandler {
    compress: Compress,
    decompress: Decompress,
    compression_level: flate2::Compression,
    min_compress_size: usize,
}

impl CompressionHandler {
    pub fn new(compression_level: flate2::Compression, min_compress_size: usize) -> Self {
        Self {
            compress: Compress::new(compression_level, false),
            decompress: Decompress::new(false),
            compression_level,
            min_compress_size,
        }
    }

    // 压缩数据
    pub fn compress(&mut self, data: &[u8]) -> Result<Bytes> {
        // 小于最小压缩大小的数据不压缩
        if data.len() < self.min_compress_size {
            return Ok(Bytes::from(data.to_vec()));
        }

        let mut output = BytesMut::with_capacity(data.len() / 2);
        let mut input = data;
        
        loop {
            let before_in = self.compress.total_in();
            let before_out = self.compress.total_out();
            
            let status = self.compress.compress(input, &mut output, FlushCompress::Finish)
                .context("压缩失败")?;
            
            let in_used = (self.compress.total_in() - before_in) as usize;
            let _out_used = (self.compress.total_out() - before_out) as usize;
            
            input = &input[in_used..];
            
            if status == flate2::Status::StreamEnd {
                break;
            }
        }
        
        self.compress.reset();
        
        // 如果压缩后数据更大，返回原始数据
        if output.len() >= data.len() {
            Ok(Bytes::from(data.to_vec()))
        } else {
            debug!("压缩成功: {} -> {} 字节", data.len(), output.len());
            Ok(output.freeze())
        }
    }

    // 解压缩数据
    pub fn decompress(&mut self, data: &[u8]) -> Result<Bytes> {
        let mut output = BytesMut::with_capacity(data.len() * 2);
        let mut input = data;
        
        loop {
            let before_in = self.decompress.total_in();
            let before_out = self.decompress.total_out();
            
            let status = self.decompress.decompress(input, &mut output, FlushDecompress::Finish)
                .context("解压缩失败")?;
            
            let in_used = (self.decompress.total_in() - before_in) as usize;
            let _out_used = (self.decompress.total_out() - before_out) as usize;
            
            input = &input[in_used..];
            
            if status == flate2::Status::StreamEnd {
                break;
            }
        }
        
        self.decompress.reset(false);
        debug!("解压缩成功: {} -> {} 字节", data.len(), output.len());
        Ok(output.freeze())
    }

    // 获取压缩级别
    pub fn compression_level(&self) -> flate2::Compression {
        self.compression_level
    }

    // 设置压缩级别
    pub fn set_compression_level(&mut self, level: flate2::Compression) {
        self.compression_level = level;
        self.compress = Compress::new(level, false);
    }

    // 获取最小压缩大小
    pub fn min_compress_size(&self) -> usize {
        self.min_compress_size
    }

    // 设置最小压缩大小
    pub fn set_min_compress_size(&mut self, size: usize) {
        self.min_compress_size = size;
    }
}

// 压缩性能统计
pub struct CompressionStats {
    pub compress_calls: usize,
    pub decompress_calls: usize,
    pub original_bytes: usize,
    pub compressed_bytes: usize,
    pub compression_ratio: f64,
}

impl CompressionStats {
    pub fn new() -> Self {
        Self {
            compress_calls: 0,
            decompress_calls: 0,
            original_bytes: 0,
            compressed_bytes: 0,
            compression_ratio: 0.0,
        }
    }
    
    pub fn record_compress(&mut self, original: usize, compressed: usize) {
        self.compress_calls += 1;
        self.original_bytes += original;
        self.compressed_bytes += compressed;
        if self.original_bytes > 0 {
            self.compression_ratio = self.compressed_bytes as f64 / self.original_bytes as f64;
        }
    }
    
    pub fn record_decompress(&mut self) {
        self.decompress_calls += 1;
    }
    
    pub fn log_stats(&self) {
        info!(
            "压缩统计: 压缩 {} 次, 解压缩 {} 次, 原始 {} 字节, 压缩后 {} 字节, 压缩率 {:.2}%",
            self.compress_calls,
            self.decompress_calls,
            self.original_bytes,
            self.compressed_bytes,
            (1.0 - self.compression_ratio) * 100.0
        );
    }
}