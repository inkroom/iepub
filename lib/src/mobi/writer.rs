use std::{
    cmp::min,
    collections::HashMap,
    io::{BufWriter, Cursor, Seek, Write},
    ops::Deref,
    path::Path,
};

use super::{
    common::{EXTHHeader, EXTHRecord, MOBIDOCHeader, MOBIHeader, PDBHeader, PDBRecordInfo},
    core::{MobiAssets, MobiBook},
    nav::generate_human_nav_xml,
};
use crate::{
    common::{IError, IResult},
    mobi::{
        common::{Record, TAGXRecord},
        image::generate_text_img_xml,
        nav::{find_chap_file_pos, generate_reader_nav_xml},
    },
    prelude::MobiNav,
};

trait WriteCount: Write {
    fn write_u16(&mut self, value: u16) -> std::io::Result<usize>;
    fn write_u32(&mut self, value: u32) -> std::io::Result<usize>;
    fn write_u64(&mut self, value: u64) -> std::io::Result<usize>;
    /// 写入指定字节数的空白，也就是0
    fn write_zero(&mut self, length: usize) -> std::io::Result<usize>;
}
impl<W: Write> WriteCount for W {
    fn write_u16(&mut self, value: u16) -> std::io::Result<usize> {
        let v: [u8; 2] = [(value >> 8) as u8, (value & 0xff) as u8];
        self.write(&v)
    }

    fn write_u32(&mut self, value: u32) -> std::io::Result<usize> {
        self.write_u16((value >> 16) as u16)?;
        self.write_u16((value & 0xffff) as u16)
    }

    fn write_u64(&mut self, value: u64) -> std::io::Result<usize> {
        self.write_u32((value >> 32) as u32)?;
        self.write_u32((value & 0xffffffff) as u32)
    }

    fn write_zero(&mut self, length: usize) -> std::io::Result<usize> {
        let v: Vec<u8> = (0..length).map(|_| 0).collect();
        self.write(&v)
    }
}
impl MOBIDOCHeader {
    fn write<T>(&self, writer: &mut T) -> IResult<()>
    where
        T: Write,
    {
        writer.write_u16(self.compression)?;

        writer.write_zero(2)?;
        writer.write_u32(self.length)?;
        writer.write_u16(self.record_count)?;
        writer.write_u16(self.record_size)?;
        writer.write_u32(0)?;

        Ok(())
    }
}
impl PDBRecordInfo {
    fn write<T>(&self, writer: &mut T) -> IResult<()>
    where
        T: Write,
    {
        writer.write_u32(self.offset)?;
        // 合并字节
        let mut v = self.unique_id;
        v |= (self.attribute as u32) << 24;
        writer.write_u32(v)?;

        Ok(())
    }
}

impl PDBHeader {
    fn write<T>(&self, writer: &mut T) -> IResult<()>
    where
        T: Write + Seek,
    {
        writer.write_all(&self.name)?;
        writer.write_u16(self.attribute)?;
        writer.write_u16(0)?; // 暂时先写成0
        writer.write_u32(self.createion_date)?;
        writer.write_u32(self.modify_date)?;
        writer.write_u32(self.last_backup_date)?;
        writer.write_u32(self.modification_number)?;
        writer.write_u32(self.app_info_id)?;
        writer.write_u32(self.sort_info_id)?;

        let _ = writer.write("BOOKMOBI".as_bytes())?;

        // writer.write_u32(self._type)?;
        // writer.write_u32(self.creator)?;
        writer.write_u32(self.unique_id_seed)?;
        writer.write_u32(self.next_record_list_id)?;
        writer.write_u16(self.number_of_records)?;

        for ele in &self.record_info_list {
            ele.write(writer)?;
        }
        writer.write_zero(2)?;

        Ok(())
    }

    fn from(title: &str, record_info_list: Vec<PDBRecordInfo>) -> Self {
        let mut name = [0u8; 32];
        // 注意编码问题
        let t = title.as_bytes();
        for i in 0..name.len() {
            if i < t.len() {
                name[i] = t[i];
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|v| v.as_secs())
            .unwrap_or(0) as u32;
        PDBHeader {
            name,
            attribute: 0,
            version: 6,
            createion_date: now,
            modify_date: now,
            last_backup_date: now,
            modification_number: 0,
            app_info_id: 0,
            sort_info_id: 0,
            _type: [0u8; 4],
            creator: [0u8; 4],
            unique_id_seed: ((2 * record_info_list.len()) - 1) as u32,
            next_record_list_id: 0,
            number_of_records: record_info_list.len() as u16,
            record_info_list,
        }
    }
}

impl MOBIHeader {
    fn write<T: Write + Seek>(
        &self,
        record0_start: u64,
        writer: &mut T,
        book: &MobiBook,
    ) -> IResult<()> {
        let start = writer.stream_position()?;
        writer.write_all("MOBI".as_bytes())?;
        writer.write_u32(self.header_len)?;
        writer.write_u32(self.mobi_type)?;
        writer.write_u32(self.text_encoding)?;
        writer.write_u32(self.unique_id)?;
        writer.write_u32(self.file_version)?;
        writer.write_u32(self.ortographic_index)?;
        writer.write_u32(self.inflection_index)?;
        writer.write_u32(self.index_names)?;
        writer.write_u32(self.index_keys)?;

        writer.write_u32(0xFFFFFFFF)?;
        writer.write_u32(0xFFFFFFFF)?;
        writer.write_u32(0xFFFFFFFF)?;
        writer.write_u32(0xFFFFFFFF)?;
        writer.write_u32(0xFFFFFFFF)?;
        writer.write_u32(0xFFFFFFFF)?;

        writer.write_u32(self.first_non_book_index)?;
        // exth 的length需要处理,写完exth后再回来更正这里的值
        let full_name_offset_index = writer.stream_position()?;
        writer.write_u32(self.full_name_offset)?;
        writer.write_u32(book.title().len() as u32)?;
        writer.write_u32(self.locale)?;
        writer.write_u32(self.input_language)?;
        writer.write_u32(self.output_language)?;
        writer.write_u32(self.min_version)?;
        writer.write_u32(self.first_image_index)?;
        writer.write_u32(self.huffman_record_offset)?;
        writer.write_u32(self.huffman_record_count)?;
        writer.write_u32(self.huffman_table_offset)?;
        writer.write_u32(self.huffman_table_length)?;
        writer.write_u32(self.exth_flags)?;
        writer.write_zero(32)?; // https://wiki.mobileread.com/wiki/MOBI#PalmDOC_Header 定义是36个字节，但是calibre只写了32个
        writer.write_u32(self.drm_offset)?;
        writer.write_u32(self.drm_count)?;
        writer.write_u32(self.drm_size)?;
        writer.write_u32(self.drm_flags)?;
        writer.write_zero(12)?; // 文档8个字节，calibre 12个字节
        writer.write_u16(self.first_content_record_number)?;
        writer.write_u16(self.last_content_record_number)?;
        writer.write_u32(1)?;
        writer.write_u32(self.fcis_record_number)?;
        writer.write_u32(1)?;
        writer.write_u32(self.flis_record_number)?;
        writer.write_u32(1)?;
        writer.write_zero(8)?;
        writer.write_u32(0xFFFFFFFF)?;
        writer.write_u32(self.first_compilation_data_section_count)?;
        writer.write_u32(self.number_of_compilation_data_sections)?;
        writer.write_u32(0xFFFFFFFF)?;
        writer.write_u32(self.extra_record_data_flags)?;
        writer.write_u32(self.indx_record_offset)?;

        // exth
        if self.exth_flags & 0x40 == 0x40 {
            EXTHHeader::from(book).write(writer)?;
        }

        let now = writer.stream_position()?;

        writer.seek(std::io::SeekFrom::Start(full_name_offset_index))?;
        writer.write_u32((now - record0_start) as u32)?;
        writer.seek(std::io::SeekFrom::Start(now))?;

        writer.write_all(book.title().as_bytes())?;
        // 添加 buffer，方便亚马逊添加加密信息
        writer.write_zero(1024 * 8)?;
        // 4字节对齐
        let now = writer.stream_position()?;
        if (now - start) % 4 != 0 {
            writer.write_zero(4 - ((now - start) % 4) as usize)?;
        }

        Ok(())
    }
}

impl EXTHHeader {
    fn from(book: &MobiBook) -> Self {
        #[inline]
        fn gene(t: crate::mobi::common::EXTHRecordType, data: &str) -> EXTHRecord {
            let v = data.as_bytes();
            EXTHRecord {
                _type: t,
                len: (8 + v.len()) as u32,
                data: v.to_vec(),
            }
        }

        let mut record_list = Vec::new();

        record_list.push(gene(
            crate::mobi::common::EXTHRecordType::UpdatedTitle,
            book.title(),
        ));
        if let Some(v) = book.publisher() {
            record_list.push(gene(crate::mobi::common::EXTHRecordType::Publisher, v));
        }

        if let Some(v) = book.creator() {
            record_list.push(gene(crate::mobi::common::EXTHRecordType::Author, v));
        }

        if let Some(v) = book.description() {
            record_list.push(gene(crate::mobi::common::EXTHRecordType::Description, v));
        }

        record_list.push(gene(super::common::EXTHRecordType::Isbn, book.identifier()));

        if let Some(v) = book.subject() {
            record_list.push(gene(super::common::EXTHRecordType::Subject, v));
        }
        if let Some(v) = book.date() {
            record_list.push(gene(super::common::EXTHRecordType::PublishingDate, v));
        }
        if let Some(v) = book.contributor() {
            record_list.push(gene(super::common::EXTHRecordType::Contributor, v));
        }
        record_list.push(EXTHRecord {
            _type: super::common::EXTHRecordType::HasFakeCover,
            len: 12,
            data: [0, 0, 0, 0].to_vec(),
        });
        record_list.push(EXTHRecord {
            _type: super::common::EXTHRecordType::CreatorSoftware,
            len: 12,
            data: [0, 0, 0, 201].to_vec(),
        });
        record_list.push(EXTHRecord {
            _type: super::common::EXTHRecordType::CreatorMajorVersion,
            len: 12,
            data: [0, 0, 0, 1].to_vec(),
        });
        record_list.push(EXTHRecord {
            _type: super::common::EXTHRecordType::CreatorMinorVersion,
            len: 12,
            data: [0, 0, 0, 2].to_vec(),
        });
        record_list.push(EXTHRecord {
            _type: super::common::EXTHRecordType::CreatorBuildNumber,
            len: 12,
            data: [0, 0, 0b10000010, 0b00011011].to_vec(), // 33307
        });

        if book.cover().is_some() {
            let len = book.assets().len() as u32;
            record_list.push(EXTHRecord {
                _type: crate::mobi::common::EXTHRecordType::CoverOffset,
                len: 12,
                data: len.to_be_bytes().to_vec(), // 封面写到最后
            });
            record_list.push(EXTHRecord {
                _type: crate::mobi::common::EXTHRecordType::ThumbOffset,
                len: 12,
                data: len.to_be_bytes().to_vec(), // 封面写到最后
            });
        }

        EXTHHeader {
            len: 0,
            record_count: record_list.len() as u32,
            record_list,
        }
    }

    fn write<T: Write + Seek>(&self, writer: &mut T) -> IResult<usize> {
        let _ = writer.write("EXTH".as_bytes())?;
        let pos = writer.stream_position()?;
        writer.write_u32(self.len)?;

        writer.write_u32(self.record_list.len() as u32)?;

        for ele in &self.record_list {
            writer.write_u32(ele._type.code())?;
            writer.write_u32(ele.len)?;
            let _ = writer.write(&ele.data)?;
        }
        let n = writer.stream_position()?;

        // 填充 对齐
        if (n - pos) % 4 != 0 {
            writer.write_zero(4 - ((n - pos) % 4) as usize)?;
        }
        let now = writer.stream_position()?;

        writer.seek(std::io::SeekFrom::Start(pos))?;
        writer.write_u32((n - pos) as u32)?;

        writer.seek(std::io::SeekFrom::Start(now))?;

        Ok((n - pos) as usize)
    }
}

impl TAGXRecord {
    fn write<T: Write + Seek>(&self, writer: &mut T) -> IResult<()> {
        writer.write("TAGX".as_bytes())?;
        writer.write_u32(self.len)?;
        writer.write_u32(self.control_byte_count)?;
        writer.write(&self.tag_table)?;

        Ok(())
    }
}

struct TAGX {
    byts: Vec<u8>,
    bitmasks: HashMap<u8, u8>,
    num_values: HashMap<u8, u8>,
}

impl TAGX {
    fn new() -> Self {
        let mut bitmasks = HashMap::new();
        bitmasks.insert(11, 0b1);
        for (i, &x) in [1, 2, 3, 4, 5, 21, 22, 23].iter().enumerate() {
            bitmasks.insert(x, 1 << i);
        }
        for (i, &x) in [69, 70, 71, 72, 73].iter().enumerate() {
            bitmasks.insert(x, 1 << i);
        }

        let mut num_values = HashMap::new();
        num_values.insert(11, 3);
        num_values.insert(0, 0);

        TAGX {
            byts: Vec::new(),
            bitmasks,
            num_values,
        }
    }

    fn add_tag(&mut self, tag: u8) {
        self.byts.push(tag);
        let value = *self.num_values.get(&tag).unwrap_or(&1);
        self.byts.push(value);
        let bitmask = if tag != 0 {
            *self.bitmasks.get(&tag).unwrap_or(&0)
        } else {
            0
        };
        self.byts.push(bitmask);
        let eof = if tag != 0 { 0 } else { 1 };
        self.byts.push(eof);
    }

    fn header(&self, control_byte_count: u32) -> Vec<u8> {
        let mut header = b"TAGX".to_vec();
        let table_length = 12 + self.byts.len() as u32;
        header.write_u32(table_length).unwrap();
        header.write_u32(control_byte_count).unwrap();
        header
    }

    fn periodical(&mut self) -> Record {
        for &i in &[1, 2, 3, 4, 5, 21, 22, 23, 0, 69, 70, 71, 72, 73, 0] {
            self.add_tag(i);
        }
        let mut result = self.header(2);
        result.extend_from_slice(&self.byts);
        result
    }

    fn secondary(&mut self) -> Record {
        for &i in &[11, 0] {
            self.add_tag(i);
        }
        let mut result = self.header(1);
        result.extend_from_slice(&self.byts);
        result
    }

    fn flat_book(&mut self) -> Record {
        for &i in &[1, 2, 3, 4, 0] {
            self.add_tag(i);
        }
        let mut result = self.header(1);
        result.extend_from_slice(&self.byts);
        result
    }
}
struct PDBRecord {
    index: usize,
    magic: Option<String>,
    data: Vec<u8>,
}

struct BookIndexEntry<'a> {
    nav: &'a MobiNav,
    offset: usize,
    size: usize,
}

fn collect_book_index<'a>(
    nav: &'a [MobiNav],
    offsets: &[NavOffset],
    body_end: usize,
) -> Vec<BookIndexEntry<'a>> {
    fn collect<'a>(
        nav: &'a [MobiNav],
        offsets: &[NavOffset],
        entries: &mut Vec<BookIndexEntry<'a>>,
    ) {
        for item in nav {
            if let Some(offset) = offsets.iter().find(|offset| offset.nav_id == item.id()) {
                entries.push(BookIndexEntry {
                    nav: item,
                    offset: offset.text_offset,
                    size: 0,
                });
            }
            collect(&item.children, offsets, entries);
        }
    }

    let mut entries = Vec::new();
    collect(nav, offsets, &mut entries);
    entries.sort_by_key(|entry| entry.offset);
    entries.dedup_by_key(|entry| entry.offset);
    for index in 0..entries.len() {
        let next_offset = entries
            .get(index + 1)
            .map_or(body_end, |entry| entry.offset);
        entries[index].size = next_offset.saturating_sub(entries[index].offset);
    }
    entries.retain(|entry| entry.size > 0);
    entries
}

/// 构建CNCX data
fn gene_cncx_data(nav: &[BookIndexEntry<'_>]) -> IResult<(Vec<Record>, HashMap<usize, u32>)> {
    let mut titles: HashMap<usize, u32> = HashMap::new();

    let record_limit = 0x10000 - 1024; //  # kindlegen appears to use 1024, PDB limit is 0x10000
    let mut out: Vec<Record> = Vec::new();
    let mut record = Vec::new();
    let mut offset = 0;

    for v in nav.iter() {
        let title = v.nav.title();
        let utf8 = title.as_bytes(); // TODO 暂不考虑超长title
        let mut raw = encode_int_forward(utf8.len() as u32);
        raw.append(&mut utf8.to_vec());
        if record.len() + raw.len() > record_limit {
            out.push(align_block(record));
            record = Vec::new();
            offset = out.len() as u32 * 0x10000;
        }
        titles.insert(v.nav.id(), offset);
        offset += raw.len() as u32;
        record.append(&mut raw);
    }
    if !record.is_empty() {
        out.push(align_block(record));
    }

    Ok((out, titles))
}
/// 填充zero到4的倍数
fn align_block(mut value: Vec<u8>) -> Vec<u8> {
    let extra = value.len() % 4;
    if extra != 0 {
        value.write_zero(4 - extra).unwrap();
    }

    value
}

/// 编码 TBS (Trailing Byte Sequence)
///
/// # 参数
/// * `val`: 主要数值
/// * `extra`: 包含额外数据和标志的 HashMap。
///   Key 是标志位掩码 (如 0b0010, 0b0100, 0b0001)
///   Value 是对应的附加数据
/// * `flag_size`: 标志位大小，默认为4
///
/// # 返回
/// 编码后的完整字节向量
fn encode_tbs(val: u64, extra: &HashMap<u8, u64>, flag_size: u32) -> Vec<u8> {
    // 1. 计算总 flags
    let mut flags: u64 = 0;
    for &flag_key in extra.keys() {
        flags |= flag_key as u64;
    }

    // 2. 编码基础的 fvwi
    let mut ans = encode_fvwi(val, flags, flag_size);

    // 3. 处理特定的额外数据
    // Python: if 0b0010 in extra: ans += encint(extra[0b0010])
    if let Some(&val_0010) = extra.get(&(0b0010 as u8)) {
        let encoded = encode_int_forward(val_0010);
        ans.extend_from_slice(&encoded);
    }

    // Python: if 0b0100 in extra: ans += bytes(bytearray([extra[0b0100]]))
    if let Some(&val_0100) = extra.get(&(0b0100 as u8)) {
        // 确保值在 u8 范围内
        if val_0100 > 0xFF {
            // return Err("Value for flag 0b0100 exceeds u8 range".to_string());
        }
        ans.push(val_0100 as u8);
    }

    // Python: if 0b0001 in extra: ans += encint(extra[0b0001])
    if let Some(&val_0001) = extra.get(&(0b0001 as u8)) {
        let encoded = encode_int_forward(val_0001);
        ans.extend_from_slice(&encoded);
    }

    ans
}

/// 编码 fvwi (Flagged Variable-Width Integer)
///
/// # 参数
/// * `val`: 主要数值
/// * `flags`: 标志位组合
/// * `flag_size`: 标志位占用的位数，默认为4
///
/// # 返回
/// 编码后的字节向量
fn encode_fvwi(val: u64, flags: u64, flag_size: u32) -> Vec<u8> {
    // 将 val 左移 flag_size 位，为 flags 腾出空间
    // 注意：需要确保移位不会溢出 u64。如果 val 很大且 flag_size 较大，可能需要 u128
    let shifted_val = val << flag_size;

    // 将 flags 的低 flag_size 位合并到 shifted_val 中
    let mask = (1u64 << flag_size) - 1;
    let combined = shifted_val | (flags & mask);

    encode_int_forward(combined)
}
/// Some parts of the Mobipocket format encode data as variable-width integers.
///    These integers are represented big-endian with 7 bits per byte in bits 1-7.
///    They may be either forward-encoded, in which case only the first byte has bit 8 set,
///    or backward-encoded, in which case only the last byte has bit 8 set.
///    For example, the number 0x11111 = 0b10001000100010001 would be represented
///    forward-encoded as `0x04 0x22 0x91 = 0b100 0b100010 0b10010001`.
///
///    Backward-encoded, it is `0x84 0x22 0x11 = 0b10000100 0b100010 0b10001`.
///
///    This function encodes the integer ``value`` as a variable width integer and
///    returns the bytestring corresponding to it.
///
///    If forward is True the bytes returned are suitable for prepending to the
///    output buffer, otherwise they must be append to the output buffer.
fn encode_int<T: Into<u128>>(value: T, forward: bool) -> Vec<u8> {
    let mut byt = Vec::new();
    let mut v = value.into();
    loop {
        let t = (v & 0b111_1111) as u8;
        v = v >> 7;
        byt.push(t);
        if v == 0 {
            break;
        }
    }

    if forward {
        byt[0] |= 0b1000_0000;
    } else {
        let l = byt.len() - 1;
        byt[l] |= 0b1000_0000;
    }
    byt.reverse();
    byt
}

fn encode_int_forward<T: Into<u128>>(value: T) -> Vec<u8> {
    encode_int(value, true)
}

fn encode_trailing_data(raw: &[u8]) -> Vec<u8> {
    let mut size_length = 1;
    let size = loop {
        let size = encode_int((raw.len() + size_length) as u64, false);
        if size.len() == size_length {
            break size;
        }
        size_length = size.len();
    };
    let mut data = raw.to_vec();
    data.extend(size);
    data
}

fn book_tbs(entries: &[BookIndexEntry<'_>], record_number: usize) -> Vec<u8> {
    let start = record_number * 4096;
    let end = start + 4096;
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    let mut completes = Vec::new();
    let mut spanner = None;
    for (index, entry) in entries.iter().enumerate() {
        let entry_end = entry.offset + entry.size;
        if entry.offset >= end || entry_end <= start {
            continue;
        }
        if entry.offset >= start {
            if entry_end <= end {
                completes.push(index);
            } else {
                starts.push(index);
            }
        } else if entry_end <= end {
            ends.push(index);
        } else {
            spanner = Some(index);
        }
    }
    if let Some(index) = spanner {
        return encode_tbs(index as u64, &HashMap::from([(0b001, 0), (0b010, 0)]), 3);
    }
    if completes.is_empty() && starts.len() + ends.len() == 1 {
        let index = starts.first().or(ends.first()).unwrap();
        return encode_tbs(*index as u64, &HashMap::from([(0b010, 0)]), 3);
    }
    let mut nodes = starts;
    nodes.extend(ends);
    nodes.extend(completes);
    nodes.sort_unstable();
    if let Some(index) = nodes.first() {
        encode_tbs(
            *index as u64,
            &HashMap::from([(0b010, 0), (0b100, nodes.len() as u64)]),
            3,
        )
    } else {
        Vec::new()
    }
}
/// 从字节开头查找是否有合法的utf8字符，有一个即可返回true，即便后面的字节可能不合法
///
/// 假设一共五个字节，前三个字节为utf8，此时即可返回true，忽略后面的字节
///
pub(crate) fn decode_utf8_ignore(value: &[u8]) -> bool {
    if value.is_empty() {
        return false;
    }
    let mut tmp = &value[..1];

    while String::from_utf8(tmp.to_vec()).is_err() {
        if tmp.len() == value.len() {
            return false;
        }
        tmp = &value[..(tmp.len() + 1)];
    }
    true
}

/// 创建一个text_record的原始数据
///
/// 返回4096长度的 text，和可能为0的额外字节，二者相加为完整的utf-8
fn create_text_record(index: usize, text: &[u8]) -> (Vec<u8>, Vec<u8>, usize) {
    let record_size = 4096;
    let pos = index;
    let next_pos = min(pos + record_size, text.len());
    let n_index = next_pos;
    let mut extra = 0;

    // 首先从4096开始往前查找，直到凑齐一个合法的utf8字符，注意此时可能包含不合法的字符

    let mut last: Vec<u8> = Vec::new();
    while !decode_utf8_ignore(&last) {
        let size = last.len() + 1;
        last.insert(0, text[next_pos - size]);
    }
    // 此时 last 应该包含一个合法的utf8字符，以及可能存在的被中途截断的utf8字节

    if String::from_utf8(last.clone()).is_err() {
        // 后面有半截字符，需要往后解析，直到字节合法
        let prev = last.len();
        let mut next = next_pos;
        loop {
            last.push(text[next]);

            if String::from_utf8(last.clone()).is_ok() {
                extra = last.len() - prev;
                break;
            }
            // 不用考虑数组越界
            next += 1;
        }
    }

    let data = text[pos..next_pos].to_vec();
    let mut overleap = Vec::new();
    for i in 0..extra {
        overleap.push(text[data.len() + pos + i]);
    }
    (data, overleap, n_index)
}
#[derive(Debug)]
struct NavOffset {
    nav_id: usize,
    text_offset: usize,
    text_size: usize,
}

impl NavOffset {
    fn next_offset(&self) -> usize {
        self.text_offset + self.text_size
    }
}

///
/// # Examples
/// ```no_run
/// use iepub::prelude::MobiWriter;
/// let fs = std::fs::OpenOptions::new()
/// .write(true)
/// .truncate(true)
/// .create(true)
/// .open("out.mobi")
/// .unwrap();
/// MobiWriter::new(fs);
/// ```
///
pub struct MobiWriter<T: Write + Seek> {
    inner: BufWriter<T>,
    /// 压缩方式，默认不压缩
    compression: u16,
    /// 是否添加标题，默认true
    append_title: bool,
    /// 首行缩进字符，默认0，不缩进
    ident: usize,
}

impl MobiWriter<std::fs::File> {
    /// 写入文件
    pub fn write_to_file<P: AsRef<Path>>(
        file: P,
        book: &MobiBook,
        append_title: bool,
    ) -> IResult<()> {
        Self::write_to_file_with_ident(file, book, append_title, 0)
    }

    /// 写入文件
    pub fn write_to_file_with_ident<P: AsRef<Path>>(
        file: P,
        book: &MobiBook,
        append_title: bool,
        ident: usize,
    ) -> IResult<()> {
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(file)
            .map_err(IError::Io)
            .map(|f| {
                MobiWriter::new(f)
                    .with_ident(ident)
                    .with_append_title(append_title)
            })
            // .map_or_else(|e| Err(IError::Io(e)), |f| MobiWriter::new(f))
            .and_then(|mut w| w.write(book))
    }
}

impl MobiWriter<std::io::Cursor<Vec<u8>>> {
    /// 写入内存
    pub fn write_to_mem(book: &MobiBook, append_title: bool) -> IResult<Vec<u8>> {
        Self::write_to_mem_with_ident(book, append_title, 0)
    }

    /// 写入内存
    pub fn write_to_mem_with_ident(
        book: &MobiBook,
        append_title: bool,
        ident: usize,
    ) -> IResult<Vec<u8>> {
        let mut v = std::io::Cursor::new(Vec::new());
        MobiWriter::new(&mut v)
            .with_ident(ident)
            .with_append_title(append_title)
            .write(book)?;

        Ok(v.into_inner())
    }
}

impl<T: Write + Seek> MobiWriter<T> {
    ///
    /// # Examples
    /// ```no_run
    /// use iepub::prelude::MobiWriter;
    /// let fs = std::fs::OpenOptions::new()
    /// .write(true)
    /// .truncate(true)
    /// .create(true)
    /// .open("out.mobi")
    /// .unwrap();
    /// let mut w = MobiWriter::new(fs);
    /// ```
    ///
    pub fn new(value: T) -> Self {
        MobiWriter {
            inner: BufWriter::new(value),
            compression: 1,
            append_title: true,
            ident: 0,
        }
    }

    pub fn set_append_title(&mut self, value: bool) {
        self.append_title = value;
    }

    pub fn with_append_title(mut self, value: bool) -> Self {
        self.set_append_title(value);
        self
    }

    pub fn set_ident(&mut self, value: usize) {
        self.ident = value;
    }

    pub fn with_ident(mut self, value: usize) -> Self {
        self.set_ident(value);
        self
    }

    fn generate_image_record(&self, assets: &[MobiAssets]) -> Vec<PDBRecord> {
        // 应该处理一下图片的
        assets
            .iter()
            .flat_map(|f| f.data())
            .enumerate()
            .map(|(index, f)| PDBRecord {
                index,
                magic: None,
                data: f.to_vec(),
            })
            .collect()
    }

    /// 序列化章节
    ///
    /// 补充html标签，修改img属性等
    fn seriable_text_html(&self, book: &MobiBook) -> (Vec<u8>, Vec<NavOffset>) {
        let mut text: Vec<u8> = Vec::new();
        text.append(
            &mut r#"<html><head><guide><reference type="toc" title="Table of Contents" filepos="#
                .as_bytes()
                .to_vec(),
        );
        let toc_pos = text.len();
        let toc_pos_len = 10;
        text.append(
            &mut format!(
                r#"{:0width$} /></guide></head><body>"#,
                0,
                width = toc_pos_len
            )
            .as_bytes()
            .to_vec(),
        );

        let mut pos = Vec::new();
        let nav = book.nav().as_slice();
        if !nav.is_empty() {
            // 添加目录html片段
            let (mut n_text, n_pos) = generate_human_nav_xml(text.len(), nav, book.title());
            pos = n_pos;
            text.append(&mut n_text);
        }
        #[inline]
        fn add_break(text: &mut Vec<u8>) {
            text.append(&mut "<mbp:pagebreak/>".as_bytes().to_vec());
        }
        let mut pos_value = HashMap::new();
        let mut nav_offset: Vec<NavOffset> = Vec::new();
        // 输出每个章节文本
        for ele in book.chapters() {
            // 修改对应的filepos
            // 可能有多个

            let pos = find_chap_file_pos(&pos, ele.id);
            for p in pos {
                pos_value.insert(ele.id, text.len());
                if let Some(last) = nav_offset.last_mut() {
                    last.text_size = text.len() - last.text_offset;
                }
                nav_offset.push(NavOffset {
                    nav_id: p.id,
                    text_offset: text.len(),
                    text_size: 0,
                });
                let pos_format = format!("{:0width$}", text.len(), width = p.length);
                for (i, v) in pos_format.as_bytes().iter().enumerate() {
                    text[p.index + i] = *v;
                }
            }

            add_break(&mut text);
            let mut v = generate_text_img_xml(
                self.html_p_ident(ele.data()).as_str(),
                &book
                    .assets()
                    .map(|f| f.file_name().to_string())
                    .collect::<Vec<String>>(),
            );
            if self.append_title && !ele.title().is_empty() {
                text.append(
                    &mut format!(r#"<h1 style="text-align: center">{}</h1>"#, ele.title())
                        .as_bytes()
                        .to_vec(),
                );
            }
            text.append(&mut v);
        }
        if let Some(last_mut) = nav_offset.last_mut() {
            last_mut.text_size = text.len() - last_mut.text_offset;
        }
        add_break(&mut text);
        // 添加结尾的目录，这部分应该是给阅读器看的

        let nav = book.nav().as_slice();
        if !nav.is_empty() {
            let p = text.len();
            let mut n_text = generate_reader_nav_xml(p, nav, &pos_value);
            text.append(&mut n_text);
            add_break(&mut text);
            // 修改 目录 定位

            let pos_format = format!("{:0width$}", p, width = toc_pos_len);
            for (i, v) in pos_format.as_bytes().iter().enumerate() {
                text[toc_pos + i] = *v;
            }
        }

        text.append(&mut "</body></html>".as_bytes().to_vec());
        (text, nav_offset)
    }

    fn html_p_ident(&self, v: Option<&[u8]>) -> String {
        if let Some(v) = v {
            let text = String::from_utf8(v.to_vec()).unwrap_or_else(|_e| String::new());
            // text.replace(from, to)
            if self.ident == 0 {
                text
            } else {
                let v = format!(r#"<p width="{}em">"#, self.ident);
                text.replace("<p ", v.as_str()).replace("<p>", v.as_str())
            }
        } else {
            String::new()
        }
    }

    /// text record
    ///
    /// record,text_length,last_text_record_idx,first_non_text_record_idx
    ///
    fn genrate_text_record(&self, text: Vec<u8>) -> (Vec<PDBRecord>, usize, usize, usize) {
        // 暂时不实行压缩

        let mut res = Vec::new();
        // 因为直接将 字节 按4096一组截取，可能出现某个编码被中间截断

        // 从尾部往前 一个字节 一个字节的连接后 按utf8解码，最后可能得情况就是字节分两组，一是一个完整编码，二是半截编码，
        let mut all_text_len = 0;
        let mut index = 0;
        while index < text.len() {
            let (mut data, over, n_index) = create_text_record(index, &text);
            index = n_index;

            all_text_len += data.len();
            data.extend_from_slice(&over);
            data.push(over.len() as u8);
            res.push(PDBRecord {
                index: res.len(),
                magic: None,
                data,
            });
        }

        // 填充间隙，确保总的字节数需要是4的倍数
        let last_text_record_idx = res.len();
        let mut first_non_text_record_idx = res.len() + 1;
        if all_text_len % 4 != 0 {
            res.push(PDBRecord {
                index: last_text_record_idx,
                magic: None,
                data: (0..(all_text_len % 4)).map(|_| 0).collect(),
            });
            first_non_text_record_idx += 1;
        }

        (
            res,
            text.len(),
            last_text_record_idx,
            first_non_text_record_idx,
        )
    }

    /// 添加结尾字节
    fn write_uncrossable_breaks(_text: Vec<PDBRecord>) -> Vec<PDBRecord> {
        todo!()
    }

    fn write_header(
        &mut self,
        book: &MobiBook,
        record_info_list: Vec<PDBRecordInfo>,
    ) -> IResult<()> {
        let s = PDBHeader::from(book.title(), record_info_list);

        s.write(&mut self.inner)
    }

    fn write_record0(
        &mut self,
        book: &MobiBook,
        text_length: usize,
        last_text_record_idx: usize,
        first_non_text_record_idx: usize,
        indx_record_offset: u32,
        last_content_record_idx: usize,
        has_index: bool,
    ) -> IResult<(usize, usize)> {
        let mobidoc_header = MOBIDOCHeader {
            compression: self.compression,
            length: text_length as u32,
            record_count: last_text_record_idx as u16,
            record_size: 4096,
            position: 0,
            encrypt_type: 0,
        };

        let start: u64 = self.inner.stream_position()?;

        mobidoc_header.write(&mut self.inner)?;

        let mobi_header = MOBIHeader {
            header_len: 0xe8,
            mobi_type: 2,
            text_encoding: 65001,
            unique_id: 98,
            file_version: 6,
            ortographic_index: 0xFFFFFFFF,
            inflection_index: 0xFFFFFFFF,
            index_names: 0xFFFFFFFF,
            index_keys: 0xFFFFFFFF,
            extra_index: [0u32; 6],
            first_non_book_index: first_non_text_record_idx as u32,
            full_name_offset: 0,
            full_name_length: book.title().len() as u32,
            locale: 9,
            input_language: 0,
            output_language: 0,
            min_version: 6,
            first_image_index: first_non_text_record_idx as u32,
            huffman_record_offset: 0,
            huffman_record_count: 0,
            huffman_table_offset: 0,
            huffman_table_length: 0,
            exth_flags: 0x40,
            drm_offset: 0xffffffff,
            drm_count: 0xffffffff,
            drm_size: 0,
            drm_flags: 0,
            first_content_record_number: 1,
            last_content_record_number: last_content_record_idx as u16,
            fcis_record_number: (last_content_record_idx + 2) as u32,
            flis_record_number: (last_content_record_idx + 1) as u32,
            first_compilation_data_section_count: 0,
            number_of_compilation_data_sections: 0xffffffff,
            extra_record_data_flags: if has_index { 0b11 } else { 0b1 },
            indx_record_offset: indx_record_offset,
        };
        mobi_header.write(start, &mut self.inner, book)?;
        let end = self.inner.stream_position()?;

        Ok((start as usize, end as usize))
    }

    pub fn write(&mut self, book: &MobiBook) -> IResult<()> {
        // 将数据拆分record,然后可以直接写入

        // record 构成为 record0(mobiheader)、image record、text record、index record(toc)

        let mut record_info_list: Vec<PDBRecordInfo> = Vec::new();
        let (text, nav_offset) = self.seriable_text_html(book);

        let (mut text, text_length, last_text_record_idx, first_non_text_record_idx) =
            self.genrate_text_record(text);
        let body_end = nav_offset.last().map_or(0, NavOffset::next_offset);
        let entries = collect_book_index(book.nav().as_slice(), &nav_offset, body_end);
        if !entries.is_empty() {
            for (record_number, record) in text.iter_mut().take(last_text_record_idx).enumerate() {
                record
                    .data
                    .extend(encode_trailing_data(&book_tbs(&entries, record_number)));
            }
        }
        let mut assets = Vec::new();

        assets.append(&mut self.generate_image_record(book.assets().as_slice()));
        if let Some(cover) = book.cover() {
            // 封面始终保持在最后一个，因为assets里可能已经包括了一份cover，在生成文本的时候recindex是按照book.assets()的顺序，放最后才能不打乱顺序
            assets.push(PDBRecord {
                index: assets.len() + text.len(),
                magic: None,
                data: cover.data().as_ref().unwrap().to_vec(),
            });
        }
        let (cncx, cncx_offset) = gene_cncx_data(&entries)?;
        // 计算INDX相关的record个数, INDX 0 + INDX 1 + CNCX
        // TODO CNCX 在超出长度的时候，可能被分成 多个record
        let indx_count = if entries.is_empty() {
            0
        } else {
            2 + cncx.len()
        };

        // 使用空数据占位，后续再来修改offset

        record_info_list.append(
            &mut (0..(text.len() + assets.len() + 4 + indx_count))
                .map(|s| PDBRecordInfo {
                    offset: 0,
                    attribute: 0,
                    unique_id: (s * 2) as u32,
                })
                .collect(),
        );

        self.write_header(book, record_info_list.clone())?;

        let last_content_record_idx = text.len() + assets.len();
        let (start, _end) = self.write_record0(
            book,
            text_length,
            last_text_record_idx,
            first_non_text_record_idx,
            if entries.is_empty() {
                0xffffffff
            } else {
                (last_content_record_idx + 3) as u32
            },
            last_content_record_idx,
            !entries.is_empty(),
        )?;

        let mut index = 0;
        record_info_list[index].offset = start as u32;

        index += 1;
        // 写入 text
        for ele in text {
            record_info_list[index].offset = self.inner.stream_position()? as u32;
            index += 1;
            self.inner.write_all(&ele.data)?;
        }

        // 写入image
        for ele in assets {
            record_info_list[index].offset = self.inner.stream_position()? as u32;
            index += 1;
            self.inner.write_all(&ele.data)?;
        }
        // 添加FCIS和FLIS
        record_info_list[index].offset = self.inner.stream_position()? as u32;
        index += 1;

        self.inner.write_all(b"FLIS\0\0\0\x08\0A\0\0\0\0\0\0\xff\xff\xff\xff\0\x01\0\x03\0\0\0\x03\0\0\0\x01\xff\xff\xff\xff")?;

        record_info_list[index].offset = self.inner.stream_position()? as u32;
        index += 1;
        self.inner.write_all(&fcis(text_length as u32))?;

        // 写入INDX
        let v = if entries.is_empty() {
            Vec::new()
        } else {
            INDX::new().generate(&entries, cncx, &cncx_offset)?
        };

        for ele in v {
            record_info_list[index].offset = self.inner.stream_position()? as u32;

            index += 1;
            self.inner.write(&ele)?;
        }

        // 还有个EOF;EOF用于确定最后一个有意义的record的边界
        record_info_list[index].offset = self.inner.stream_position()? as u32;
        self.inner.write_all(b"\xE9\x8E\x0D\x0A")?;

        // 重新写入offset
        self.inner.seek(std::io::SeekFrom::Start(78))?;
        for ele in &record_info_list {
            ele.write(&mut self.inner)?;
        }

        Ok(())
    }
}
fn fcis(text_length: u32) -> Vec<u8> {
    let mut fcis = Vec::new();

    // 添加固定字节序列
    fcis.extend_from_slice(b"FCIS\x00\x00\x00\x14\x00\x00\x00\x10\x00\x00\x00\x01\x00\x00\x00\x00");

    // 添加大端序的text_length
    fcis.extend_from_slice(&(text_length).to_be_bytes());

    // 添加剩余的固定字节序列
    fcis.extend_from_slice(
        b"\x00\x00\x00\x00\x00\x00\x00\x20\x00\x00\x00\x08\x00\x01\x00\x01\x00\x00\x00\x00",
    );

    fcis
}
struct INDX {}

impl INDX {
    fn new() -> Self {
        Self {}
    }
    ///
    /// 需要章节名和正文的offset映射，key是章节名
    ///
    fn generate(
        &self,
        entries: &[BookIndexEntry<'_>],
        mut cncx_data: Vec<Record>,
        cncx_offset: &HashMap<usize, u32>,
    ) -> IResult<Vec<Record>> {
        let mut res = Vec::new();
        res.push(self.write_indx0(entries.len(), cncx_data.len())?);
        res.push(self.write_indx1(entries, cncx_offset)?);

        res.append(&mut cncx_data);

        Ok(res)
    }

    fn write_indx0(&self, nav_len: usize, num_of_cncx: usize) -> IResult<Record> {
        let mut writer = BufWriter::new(Cursor::new(Record::new()));
        let header_length = 192;
        writer.write(b"INDX")?;
        writer.write_u32(header_length)?;
        writer.write_zero(8)?;
        // Index type: 0 - normal, 2 - inflection
        writer.write_u32(2)?;
        let idxt_offset_postion = writer.stream_position()?;
        //  IDXT offset
        writer.write_u32(0)?;
        //  Number of index records
        writer.write_u32(1)?;

        writer.write_u32(65001)?;
        writer.write_u32(0xFFFFFFFF)?;

        let nav_len = nav_len as u32;
        // Number of index entries
        // TODO 兼容多级目录
        writer.write_u32(nav_len)?;
        // ORDT offset
        writer.write_zero(4)?;
        // LIGT offset
        writer.write_zero(4)?;
        // Number of LIGT entries
        writer.write_u32(0)?;
        //  Number of CNCX records
        writer.write_u32(num_of_cncx as u32)?;

        writer.write_zero(124)?;
        // TAGX offset
        writer.write_u32(header_length)?;
        writer.write_zero(8)?;

        // 写入TAGX
        let tagx_block = TAGX::new().flat_book();
        writer.write_all(tagx_block.as_slice())?;

        // NCX
        writer.write(&encode_number_as_hex(nav_len - 1))?;
        // The number of entries in the NCX
        writer.write_u16(nav_len as u16)?;

        // writer = align_block(writer);

        // Padding
        let pad = 4 - writer.stream_position()? % 4;
        writer.write_zero(pad as usize)?;

        // idxt
        let idxt = writer.stream_position()?;

        writer.write(b"IDXT")?;
        writer.write_u16((header_length + tagx_block.len() as u32) as u16)?;
        writer.write_zero(1)?;

        writer.seek(std::io::SeekFrom::Start(idxt_offset_postion as u64))?;
        writer.write_u32(idxt as u32)?;

        writer
            .into_inner()
            .map(|v| v.into_inner())
            .map_err(|e| IError::Io(e.into_error()))
            .map(|f| align_block(f))
    }

    fn write_indx1(
        &self,
        entries: &[BookIndexEntry<'_>],
        cncx_offset: &HashMap<usize, u32>,
    ) -> IResult<Record> {
        let mut writer = BufWriter::new(Cursor::new(Record::new()));

        let header_length = 192;
        writer.write(b"INDX")?;
        writer.write_u32(header_length)?;
        writer.write_zero(4)?;
        // Header type? Or index record number?
        writer.write_u32(1)?;
        writer.write_zero(4)?;
        let idxt_block_offset = writer.stream_position()?;
        // IDXT block offset
        writer.write_u32(0)?;
        writer.write_u32(entries.len() as u32)?;
        writer.write_u64(0xFFFFFFFFFFFFFFFF)?;
        writer.write_zero(156)?;

        // INDEX block

        let tagx = TAGX::new();
        let before = writer.stream_position()?;
        let mut index_offset = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            index_offset.push(writer.stream_position()?);
            writer.write(&encode_number_as_hex(index as u32))?;
            let mut ans = 0;
            for i in 1..5 {
                ans = ans | tagx.bitmasks[&i];
            }

            writer.write(&[ans])?;
            for i in 1..5 {
                if i == 1 {
                    // offset ,正文的偏移
                    writer.write(&encode_int_forward(entry.offset as u32))?;
                } else if i == 2 {
                    // size 正文的size
                    writer.write(&encode_int_forward(entry.size as u32))?;
                } else if i == 3 {
                    // label_offset 章节的offset
                    writer.write(&encode_int_forward(cncx_offset[&entry.nav.id()]))?;
                } else if i == 4 {
                    // depth 缩进
                    writer.write(&encode_int_forward(0 as u32))?;
                }
            }
        }
        // Padding
        let pad = 4 - ((writer.stream_position()? - before) % 4);
        writer.write_zero(pad as usize)?;

        // IDXT
        let before = writer.stream_position()?;
        writer.write(b"IDXT")?;
        for ele in index_offset {
            writer.write_u16((ele) as u16)?;
        }

        let pad = 4 - ((writer.stream_position()? - before) % 4);
        writer.write_zero(pad as usize)?;

        writer.seek(std::io::SeekFrom::Start(idxt_block_offset))?;
        writer.write_u32(before as u32)?;

        writer
            .into_inner()
            .map(|v| v.into_inner())
            .map_err(|e| IError::Io(e.into_error()))
            .map(|f| align_block(f))
    }
}

/// 将原始字节数组编码为：长度（1字节）+ 内容
fn encode_string(raw: &[u8]) -> Vec<u8> {
    let mut ans = Vec::with_capacity(raw.len() + 1);
    ans.push(raw.len() as u8);
    ans.extend_from_slice(raw);
    ans
}

/// 将数字编码为变长十六进制数
/// 编码格式：长度（1字节）+ 十六进制表示（ASCII编码）
fn encode_number_as_hex(num: u32) -> Vec<u8> {
    // 将数字转换为十六进制字符串（大写）
    let hex_str = format!("{:X}", num);

    // 确保十六进制字符串长度为偶数
    if hex_str.len() % 2 == 0 {
        encode_string(hex_str.as_bytes())
    } else {
        // 在奇数长度前补0
        let mut padded = String::with_capacity(hex_str.len() + 1);
        padded.push('0');
        padded.push_str(&hex_str);
        encode_string(padded.as_bytes())
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        mobi::writer::decode_utf8_ignore,
        prelude::{MobiBuilder, MobiHtml, MobiNav, MobiReader},
    };

    use super::{BookIndexEntry, MobiWriter, book_tbs, encode_trailing_data};

    #[test]
    fn test_book_tbs_record_boundaries() {
        let first = MobiNav::new(1, 1);
        let second = MobiNav::new(2, 2);
        let entries = [
            BookIndexEntry {
                nav: &first,
                offset: 100,
                size: 5000,
            },
            BookIndexEntry {
                nav: &second,
                offset: 5100,
                size: 100,
            },
        ];
        assert_eq!(book_tbs(&entries, 0), [0x82, 0x80]);
        assert_eq!(book_tbs(&entries, 1), [0x86, 0x80, 2]);
        assert!(book_tbs(&entries, 2).is_empty());
        assert_eq!(
            encode_trailing_data(&book_tbs(&entries, 1)),
            [0x86, 0x80, 2, 0x84]
        );
    }

    #[test]
    fn test_kindle_navigation_index() {
        let data = MobiBuilder::new()
            .with_title("Navigation")
            .cover(vec![1, 2, 3])
            .add_chapter(
                MobiHtml::new(1)
                    .with_title("First")
                    .with_data(format!("<p>{}</p>", "text".repeat(1200)).into_bytes()),
            )
            .add_chapter(
                MobiHtml::new(2)
                    .with_title("Second")
                    .with_data(b"<p>Second chapter</p>".to_vec()),
            )
            .mem()
            .unwrap();
        let be_u32 =
            |offset: usize| u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap());
        let record_offset = |number: usize| be_u32(78 + number * 8) as usize;
        let record0 = record_offset(0);
        let text_count =
            u16::from_be_bytes(data[record0 + 8..record0 + 10].try_into().unwrap()) as usize;
        assert_eq!(be_u32(record0 + 16 + 0xe0), 0b11);
        let index_record = be_u32(record0 + 16 + 0xe4) as usize;
        assert_eq!(
            &data[record_offset(index_record)..record_offset(index_record) + 4],
            b"INDX"
        );
        let entry_record = record_offset(index_record + 1);
        assert_eq!(be_u32(entry_record + 4), 192);
        assert_eq!(
            &data[entry_record + 192..entry_record + 195],
            &[2, b'0', b'0']
        );
        assert_eq!(be_u32(entry_record + 24), 2);
        let entry_index = entry_record + be_u32(entry_record + 20) as usize;
        let second_entry = entry_record
            + u16::from_be_bytes(data[entry_index + 6..entry_index + 8].try_into().unwrap())
                as usize;
        let mut cursor = second_entry + 1 + data[second_entry] as usize + 1;
        let mut read_number = || {
            let mut value = 0u32;
            loop {
                let byte = data[cursor];
                cursor += 1;
                value = (value << 7) | (byte & 0x7f) as u32;
                if byte & 0x80 != 0 {
                    return value;
                }
            }
        };
        read_number();
        read_number();
        let label_offset = read_number() as usize;
        let cncx_record = record_offset(index_record + 2);
        assert_eq!(data[cncx_record + label_offset], 0x89);
        assert_eq!(
            &data[cncx_record + label_offset + 1..cncx_record + label_offset + 10],
            b"2. Second"
        );
        assert!((1..=text_count).any(|number| {
            let record = &data[record_offset(number)..record_offset(number + 1)];
            let trailing_size = (record[record.len() - 1] & 0x7f) as usize;
            trailing_size > 1 && trailing_size <= record.len()
        }));
    }

    #[test]
    #[should_panic]
    fn test_utf8() {
        let v = "中文".as_bytes();

        println!("v.{}", v.len());

        assert_eq!(true, decode_utf8_ignore(v));

        let mut m = v.to_vec();
        m.push("中文".as_bytes()[0]);
        assert_eq!(true, decode_utf8_ignore(&m));

        println!("{}", String::from_utf8(m).unwrap());
    }

    #[test]
    #[ignore = "dan.mobi"]
    fn test_write() {
        {
            let fs = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open("demo.mobi")
                .unwrap();

            let mut w = MobiWriter::new(fs);

            let path = std::env::current_dir().unwrap().join("../dan.mobi");
            let mut mobi =
                MobiReader::new(std::fs::File::open(path.to_str().unwrap()).unwrap()).unwrap();

            let book = mobi.load().unwrap();

            w.write(&book).unwrap();
        }
        {
            // let mut fs = std::fs::OpenOptions::new()
            //     .read(true)
            //     .open("demo.mobi")
            //     .unwrap();
            // fs.seek(std::io::SeekFrom::Start(5336 + 16)).unwrap();
            // let h = MOBIHeader::load(&mut fs).unwrap();

            // assert_eq!(3, h.extra_record_data_flags);
        }
    }
}
