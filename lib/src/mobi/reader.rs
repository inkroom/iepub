//!
//! 支持mobi格式。
//!
//! mobi是采用[PDB](https://wiki.mobileread.com/wiki/PDB#Intro_to_the_Database_format)格式封装，就像epub使用zip格式封装一样
//!

use std::{
    borrow::Cow,
    collections::HashMap,
    io::{BufReader, Read, Seek, SeekFrom},
    sync::atomic::AtomicUsize,
};

use crate::{
    common::{BookInfo, IError, IResult},
    mobi::core::MobiNav,
};

use super::{
    common::{
        EXTHHeader, EXTHRecord, INDXRecord, MOBIDOCHeader, MOBIHeader, PDBHeader, PDBRecordInfo,
        NCX,
    },
    core::MobiAssets,
    image::{get_suffix, read_image_recindex_from_html, Cover},
    nav::{read_guide_filepos, read_nav_xml},
};

fn vec_u8_to_u64(v: &[u8]) -> u64 {
    let mut u64: u64 = 0;
    for ele in v {
        u64 = u64 << 8;
        u64 = u64 | (*ele as u64)
    }
    u64
}

impl PDBHeader {
    fn load<T>(reader: &mut T) -> IResult<Self>
    where
        T: Read + Seek,
    {
        let mut header = PDBHeader::default();
        // header.name = reader.read_string(32)?;

        reader.read_exact(&mut header.name)?;

        header.attribute = reader.read_u16()?;
        header.version = reader.read_u16()?;
        header.createion_date = reader.read_u32()?;
        header.modify_date = reader.read_u32()?;
        header.last_backup_date = reader.read_u32()?;
        header.modification_number = reader.read_u32()?;
        header.app_info_id = reader.read_u32()?;
        header.sort_info_id = reader.read_u32()?;

        if "BOOKMOBI" != reader.read_string(8)? {
            return Err(IError::UnsupportedArchive("not a mobi file"));
        }

        // header._type = reader.read_u32()?;
        // header.creator = reader.read_u32()?;
        header.unique_id_seed = reader.read_u32()?;
        header.next_record_list_id = reader.read_u32()?;
        header.number_of_records = reader.read_u16()?;

        let mut record_info_list: Vec<PDBRecordInfo> = vec![];
        // 读取header
        for _ in 0..header.number_of_records {
            let mut info = PDBRecordInfo::default();
            info.offset = reader.read_u32()?;

            let a = reader.read_u32()?;
            // 取最高位的
            info.attribute = (a >> 24) as u8;
            // 去掉最高8位
            info.unique_id = a & 0x00ffffff;
            record_info_list.push(info);
        }
        header.record_info_list = record_info_list;
        Ok(header)
    }
}

impl MOBIDOCHeader {
    fn load<T>(reader: &mut T, offset: u64) -> IResult<Self>
    where
        T: Read + Seek,
    {
        reader.seek(SeekFrom::Start(offset))?;

        let mut mo = MOBIDOCHeader::default();
        mo.compression = reader.read_u16()?;
        reader.read_u16()?;
        mo.length = reader.read_u32()?;
        mo.record_count = reader.read_u16()?;
        mo.record_size = reader.read_u16()?;
        mo.position = reader.read_u32()?;
        if mo.compression == 17480 {
            mo.encrypt_type = ((mo.position >> 8) & 0xffff) as u16;
        }
        Ok(mo)
    }
}

impl MOBIHeader {
    pub fn load<T>(reader: &mut T) -> IResult<Self>
    where
        T: Read + Seek,
    {
        let mut header = Self::default();

        let start = reader.stream_position()?;

        if "MOBI" != reader.read_string(4)? {
            return Err(IError::UnsupportedArchive("not a mobi file"));
        }

        header.header_len = reader.read_u32()?;
        header.mobi_type = reader.read_u32()?;
        header.text_encoding = reader.read_u32()?;
        header.unique_id = reader.read_u32()?;
        header.file_version = reader.read_u32()?;
        header.ortographic_index = reader.read_u32()?;
        header.inflection_index = reader.read_u32()?;
        header.index_names = reader.read_u32()?;
        header.index_keys = reader.read_u32()?;
        reader.read_exact_u32(&mut header.extra_index)?;
        header.first_non_book_index = reader.read_u32()?;
        // 规范里要求 offset 是从 record 0 开始，也就是当前这个 mobi header，但是为了方便，这里给改成从 文件开头开始索引
        header.full_name_offset = reader.read_u32()?;
        header.full_name_offset += start as u32;
        // palm_doc 的16个字节也在 record 0 里面，所以还需要去掉
        header.full_name_offset -= 16;

        header.full_name_length = reader.read_u32()?;
        header.locale = reader.read_u32()?;
        header.input_language = reader.read_u32()?;
        header.output_language = reader.read_u32()?;
        header.min_version = reader.read_u32()?;
        header.first_image_index = reader.read_u32()?;
        header.huffman_record_offset = reader.read_u32()?;
        header.huffman_record_count = reader.read_u32()?;
        header.huffman_table_offset = reader.read_u32()?;
        header.huffman_table_length = reader.read_u32()?;
        header.exth_flags = reader.read_u32()?;

        reader.seek(SeekFrom::Current(32))?;
        // reader.read_exact(&mut header.unknown_0)?;

        let _ = reader.read_u32()?;
        header.drm_offset = reader.read_u32()?;
        header.drm_count = reader.read_u32()?;
        header.drm_size = reader.read_u32()?;
        header.drm_flags = reader.read_u32()?;
        let _ = reader.read_u64()?;
        header.first_content_record_number = reader.read_u16()?;
        header.last_content_record_number = reader.read_u16()?;
        let _ = reader.read_u32()?;
        header.fcis_record_number = reader.read_u32()?;
        let _ = reader.read_u32()?;
        header.flis_record_number = reader.read_u32()?;
        let _ = reader.read_u32()?;
        let _ = reader.read_u64()?;
        let _ = reader.read_u32()?;
        header.first_compilation_data_section_count = reader.read_u32()?;
        header.number_of_compilation_data_sections = reader.read_u32()?;
        let _ = reader.read_u32()?;
        header.extra_record_data_flags = reader.read_u32()?;
        header.indx_record_offset = reader.read_u32()?;

        // 有的 mobi header长度是256，有的232，所以有可能需要跳过一些字节
        reader.seek(SeekFrom::Start(start + header.header_len as u64))?;
        Ok(header)
    }
}

impl EXTHRecord {
    fn load<T: ReadCount>(reader: &mut T) -> IResult<Self> {
        let mut v = Self::default();
        v._type = reader.read_u32()?;
        v.len = reader.read_u32()?;

        reader.take((v.len - 8) as u64).read_to_end(&mut v.data)?;

        Ok(v)
    }
}

macro_rules! simple_utf8 {
    ($expr:expr) => {{
        match String::from_utf8($expr.clone()) {
            Ok(va) => $crate::common::unescape_html(va.as_str()),
            Err(e) => {
                return Err($crate::common::IError::Utf8(e));
            }
        }
    }};
}

impl EXTHHeader {
    ///
    /// [reader] reader
    /// [exth_flas] 如果 &0x40 != 0x40，返回None
    pub fn load<T>(reader: &mut T, exth_flags: u32) -> IResult<Option<Self>>
    where
        T: ReadCount,
    {
        if exth_flags & 0x40 != 0x40 {
            return Ok(None);
        }

        let mut v = Self::default();

        if "EXTH" != reader.read_string(4)? {
            return Err(IError::InvalidArchive(Cow::from("not a exth")));
        }

        v.len = reader.read_u32()?;
        v.record_count = reader.read_u32()?;
        for _ in 0..v.record_count {
            v.record_list.push(EXTHRecord::load(reader)?);
        }

        // 跳过一定padding字节数,规则是保证 header整体长度一定是4的整数，如果实际数据不够，则填充到4的整数
        // Null bytes to pad the EXTH header to a multiple of four bytes (none if the header is already a multiple of four). This padding is not included in the EXTH header length.
        let skip = 4 - v.len % 4;
        if skip != 4 {
            reader.seek(SeekFrom::Current(skip as i64))?;
        }
        Ok(Some(v))
    }

    fn get_cover_offset(&self) -> Option<u64> {
        self.record_list
            .iter()
            .find(|x| x._type == 201)
            .map(|f| vec_u8_to_u64(&f.data))
            .filter(|f| f < &0xffffffff)
    }

    fn get_thumbnail_offset(&self) -> Option<u64> {
        self.record_list
            .iter()
            .find(|x: &&EXTHRecord| x._type == 202)
            .map(|f| vec_u8_to_u64(&f.data))
            .filter(|f| f < &0xffffffff)
    }

    /// 解析元数据
    fn get_meta(&self) -> IResult<BookInfo> {
        let mut info = BookInfo::default();
        for ele in &self.record_list {
            match ele._type {
                100 => {
                    let v = simple_utf8!(ele.data);
                    // 暂时只考虑utf-8编码
                    info.append_creator(v.as_str());
                }
                101 => {
                    info.publisher = Some(simple_utf8!(ele.data));
                }
                103 => {
                    info.description = Some(simple_utf8!(ele.data));
                }
                104 => {
                    info.identifier = simple_utf8!(ele.data);
                }
                105 => {
                    info.subject = Some(simple_utf8!(ele.data));
                }
                106 => {
                    info.date = Some(simple_utf8!(ele.data));
                }
                108 => {
                    info.contributor = Some(simple_utf8!(ele.data));
                }
                503 => {
                    info.title = simple_utf8!(ele.data);
                }
                _ => {}
            }
        }
        Ok(info)
    }
}

impl INDXRecord {
    pub fn load<T>(reader: &mut T) -> IResult<Self>
    where
        T: ReadCount,
    {
        let mut v = Self::default();

        if reader.read_string(4)? != "INDX" {
            return Err(IError::InvalidArchive(Cow::from("not a indx")));
        }

        v.len = reader.read_u32()?;
        v._type = reader.read_u32()?;
        reader.skip(8)?;
        v.idxt_start = reader.read_u32()?;
        v.index_count = reader.read_u32()?;
        v.index_encoding = reader.read_u32()?;
        v.index_language = reader.read_u32()?;
        v.total_index_count = reader.read_u32()?;
        v.ordt_start = reader.read_u32()?;
        v.ligt_start = reader.read_u32()?;
        v.ligt_count = reader.read_u32()?;
        v.cncx_count = reader.read_u32()?;

        // 整个indx不止文档里的56个字节，多出的长度应该就是 index value
        reader.skip((v.len - 56).into())?;
        // reader.seek(SeekFrom::Start(start + v.len as u64))?;

        Ok(v)
    }
}

/// 计算一个数字 有多少位是1
fn count_bit(v: u32) -> usize {
    let mut count = 0;
    let mut nv = v;
    while (nv) > 0 {
        if (nv & 1) == 1 {
            count += 1;
        }
        nv = nv >> 1;
    }
    return count;
}
/// 统计有多少位0 ，因为不同类型循环次数不同，会非常影响结果
fn count_unset_end(v: u8) -> usize {
    let mut count = 0;
    let mut x = v;
    while (x & 1) == 0 {
        x = x >> 1;
        count += 1;
    }
    count
}
fn get_var_len(byte: &[u8]) -> (usize, usize) {
    let mut value: usize = 0;
    let mut length: usize = 0;
    for ele in byte {
        value = (value << 7) | ((ele & 0b111_1111) as usize);
        length += 1;
        if ele & 0b1000_0000 >= 1 {
            break;
        }
    }
    (value, length)
}

// https://wiki.mobileread.com/wiki/PDB#Intro_to_the_Database_format
// https://wiki.mobileread.com/wiki/PDB#Palm_Database_Format

// fn u32_to_string(value:[u32])->String{

//     let mut v = [0u8;4];
//     v[0] = (value >> 24 & 0xff) as u8;
//     v[1]=(value >> 16 & 0xff) as u8;
//     v[2] = (value >> 8 & 0xff) as u8;
//     v[3] = (value & 0xff) as u8;

//     String::from_utf8(v.to_vec()).unwrap_or(String::new())

// }

mod ext {
    use std::{
        collections::HashMap,
        io::{Read, Seek, SeekFrom},
    };

    use crate::common::{IError, IResult};

    pub(crate) trait NCXExt {
        fn get_value(&self, index: u8) -> Option<usize>;
        fn get_value_or(&self, index: u8, default: usize) -> usize;
    }

    impl NCXExt for HashMap<u8, Vec<usize>> {
        fn get_value(&self, index: u8) -> Option<usize> {
            self.get(&index).and_then(|f| f.get(0)).map(|f| f.clone())
        }
        fn get_value_or(&self, index: u8, default: usize) -> usize {
            self.get_value(index).unwrap_or(default)
        }
    }
    pub(crate) trait ReadCount: Read + Seek {
        fn read_u8(&mut self) -> IResult<u8> {
            let mut out = [0u8; 1];
            self.read_exact(&mut out)?;
            Ok(out[0])
        }
        fn read_u16(&mut self) -> IResult<u16> {
            let mut out = [0u8; 2];
            let mut res: u16 = 0;
            self.read_exact(&mut out)?;
            for i in 0..out.len() {
                res = res << 8;
                res = res | out[i] as u16;
            }
            Ok(res)
        }
        fn read_u32(&mut self) -> IResult<u32> {
            let mut out = [0u8; 4];
            let mut res: u32 = 0;
            self.read_exact(&mut out)?;
            for i in 0..out.len() {
                res = res << 8;
                res = res | out[i] as u32;
            }

            Ok(res)
        }
        fn read_u64(&mut self) -> IResult<u64> {
            let mut out = [0u8; 8];
            let mut res: u64 = 0;
            self.read_exact(&mut out)?;

            for i in 0..out.len() {
                res = res << 8;
                res = res | out[i] as u64;
            }

            Ok(res)
        }

        fn read_exact_u32<const N: usize>(&mut self, value: &mut [u32; N]) -> IResult<()> {
            for i in 0..value.len() {
                value[i] = self.read_u32()?;
            }
            Ok(())
        }

        fn read_string(&mut self, limit: u64) -> IResult<String> {
            let mut out = String::new();
            self.take(limit).read_to_string(&mut out)?;

            Ok(out)
        }

        fn skip(&mut self, limit: u64) -> IResult<u64> {
            self.seek(SeekFrom::Current(limit as i64))
                .map_err(|f| IError::Io(f))
        }
    }

    impl<R: Read + Seek> ReadCount for R {}
}

use ext::ReadCount;

impl NCX {
    fn from(index: usize, label: String, map: &HashMap<u8, Vec<usize>>) -> Self {
        use ext::NCXExt;

        NCX {
            index,
            offset: map.get_value(1),
            size: map.get_value(2),
            label,
            heading_lebel: map.get_value_or(4, 0),
            pos: map.get_value_or(6, 0),
            parent: map.get_value(21),
            first_child: map.get_value(22),
            last_child: map.get_value(23),
        }
    }
}
/// 判断是否是mobi
pub fn is_mobi<T>(value: &mut T) -> IResult<bool>
where
    T: Read + Seek,
{
    value.seek(SeekFrom::Start(60))?;
    let mut buf = Vec::new();
    let _ = value.take(8).read_to_end(&mut buf)?;

    if buf != b"BOOKMOBI" {
        return Ok(false);
    }
    Ok(true)
}

pub struct MobiReader<T> {
    reader: BufReader<T>,
    pub(crate) pdb_header: PDBHeader,
    pub(crate) mobi_doc_header: MOBIDOCHeader,
    pub(crate) mobi_header: MOBIHeader,
    pub(crate) exth_header: Option<EXTHHeader>,
    /// 原始文本缓存
    text_cache: Option<Vec<u8>>,
    /// 自增id
    id: AtomicUsize,
}

impl<T: Read + Seek> MobiReader<T> {
    pub fn new(v: T) -> IResult<MobiReader<T>> {
        // let fs = std::fs::File::open(file)?;
        let mut reader = BufReader::new(v);

        // 校验基础格式
        reader.seek(SeekFrom::Start(60))?;

        if reader.read_string(8)? != "BOOKMOBI" {
            return Err(IError::InvalidArchive(Cow::from("not a mobi")));
        }
        reader.seek(SeekFrom::Start(0))?;

        let pdb_header = PDBHeader::load(&mut reader)?;
        let mobi_doc_header =
            MOBIDOCHeader::load(&mut reader, pdb_header.record_info_list[0].offset as u64)?;
        let mobi_header = MOBIHeader::load(&mut reader)?;

        let exth_header = EXTHHeader::load(&mut reader, mobi_header.exth_flags)?;

        Ok(MobiReader {
            reader,
            pdb_header,
            mobi_doc_header,
            mobi_header,
            exth_header,
            text_cache: None,
            id: AtomicUsize::new(1),
        })
    }

    /// 解析书籍元数据
    pub(crate) fn read_meta_data(&mut self) -> IResult<BookInfo> {
        let current = self.reader.stream_position()?;

        self.reader
            .seek(SeekFrom::Start(self.mobi_header.full_name_offset as u64))?;
        let mut title = String::new();

        self.reader
            .get_mut()
            .take(self.mobi_header.full_name_length as u64)
            .read_to_string(&mut title)?;
        self.reader.seek(SeekFrom::Start(current))?;
        // self.reader.read_to_string(buf)

        if let Some(exth) = &self.exth_header {
            return exth.get_meta();
        }
        let mut info = BookInfo::default();
        info.title = title;
        Ok(info)
    }
    /// record的第一个字节的offset
    ///
    /// (当前的offset，下一个的offset)
    ///
    pub(crate) fn seek_record_offset(&mut self, index: u32) -> IResult<(u64, u64)> {
        let offset = self.pdb_header.record_info_list[index as usize].offset as u64;
        self.reader.seek(SeekFrom::Start(offset))?;

        Ok((
            offset,
            self.pdb_header.record_info_list[(index + 1) as usize].offset as u64,
        ))
    }

    /// 从文本中获取目录信息
    /// [sec] 分节信息，filepos
    pub(crate) fn read_nav_from_text(
        &mut self,
        sec: &[TextSection],
    ) -> IResult<Option<Vec<MobiNav>>> {
        let raw = self.read_text_raw()?;
        let file_pos = read_guide_filepos(&raw[..])?;

        if let Some(toc) = file_pos.map_or(None, |v| sec.iter().find(|s| s.end > v)) {
            return read_nav_xml(toc.data.as_bytes().to_vec(), &mut self.id).map(|s| Some(s));
        }

        Ok(None)
    }

    /// 解析目录
    pub(crate) fn read_nav(&mut self) -> IResult<()> {
        if self.mobi_header.indx_record_offset < 0xffffffff {
            self.seek_record_offset(self.mobi_header.indx_record_offset)?;

            let indx = INDXRecord::load(&mut self.reader)?;

            if self.reader.read_string(4)? != "TAGX" {
                return Err(IError::InvalidArchive(Cow::from("not a tagx")));
            }
            let mut tagx_table: Vec<[u8; 4]> = Vec::new();
            let len = self.reader.read_u32()?;
            // the number of control bytes
            let tagx_control_byte_count = self.reader.read_u32()?;

            for _ in 0..((len - 12) / 4) {
                // 四个字节的含义
                // The tag table entries are multiple of 4 bytes. The first byte is the tag, the second byte the number of values, the third byte the bit mask and the fourth byte indicates the end of the control byte. If the fourth byte is 0x01, all other bytes of the entry are zero.
                let mut v = [0u8; 4];
                self.reader.read_exact(&mut v)?;

                tagx_table.push(v);
            }

            // 剩余字段的解析方式文档里没有再多描述，只能翻译别的项目代码
            let mut cntx = HashMap::new();
            let mut cncx_record_offset = 0;
            for i in 0..indx.cncx_count {
                let (now, offset) = self.seek_record_offset(
                    self.mobi_header.indx_record_offset + indx.index_count + 1 + i,
                )?;

                let mut record = Vec::new();
                self.reader
                    .get_mut()
                    .take(offset - now)
                    .read_to_end(&mut record)?;

                let mut pos = 0;

                while pos < record.len() {
                    let index = pos;
                    let bytes = &record[pos..(pos + 4)];
                    let (value, length) = get_var_len(&bytes[0..]);
                    pos += length;
                    let result = &record[pos..(pos + value)];
                    pos += value;
                    cntx.insert(
                        cncx_record_offset + index,
                        String::from_utf8(result.to_vec()).unwrap_or(String::new()),
                    );
                }
                cncx_record_offset += 0x10000;
            }

            let mut table = Vec::new();
            for i in 0..indx.index_count {
                let (start, _) =
                    self.seek_record_offset(self.mobi_header.indx_record_offset + 1 + i)?;
                let n_index = INDXRecord::load(&mut self.reader)?;

                for j in 0..n_index.index_count {
                    let offset_offset = (n_index.idxt_start + 4 + 2 * j) as u64;
                    self.reader.seek(SeekFrom::Start(start + offset_offset))?;
                    let offset = self.reader.read_u16()? as u64;
                    self.reader.seek(SeekFrom::Start(start + offset as u64))?;

                    let length = self.reader.read_u8()?;
                    self.reader
                        .seek(SeekFrom::Start(start + offset as u64 + 1))?;
                    let _name = self.reader.read_string(length as u64)?;

                    let mut tags = Vec::new();

                    let start_pos = offset + 1 + length as u64;
                    let mut control_byte_index = 0;
                    let mut pos = start_pos + tagx_control_byte_count as u64;
                    #[inline]
                    fn get_array_var_len(
                        reader: &mut impl ReadCount,
                        start: u64,
                        pos: u64,
                    ) -> IResult<(usize, usize)> {
                        reader.seek(SeekFrom::Start(start + pos))?;
                        let mut buf = [0u8; 4];
                        reader.read_exact(&mut buf)?;
                        Ok(get_var_len(&buf))
                    }

                    for ele in &tagx_table {
                        let tag = ele[0];
                        let num_values = ele[1];
                        let mask = ele[2];
                        let end = ele[3];

                        if end & 1 >= 1 {
                            control_byte_index += 1;
                            continue;
                        }

                        let offset: u64 = start_pos + control_byte_index;
                        self.reader.seek(SeekFrom::Start(start + offset))?;
                        let value = self.reader.read_u8()? & mask;
                        if value == mask {
                            if count_bit(mask as u32) > 1 {
                                let (value, length) =
                                    get_array_var_len(&mut self.reader, start, pos)?;

                                tags.push((tag, None, Some(value), num_values));
                                pos += length as u64;
                            } else {
                                tags.push((tag, Some(1), None, num_values));
                            }
                        } else {
                            tags.push((
                                tag,
                                Some(value >> count_unset_end(mask)),
                                None,
                                num_values,
                            ));
                        }
                    }

                    let mut tag_map: HashMap<u8, Vec<usize>> = HashMap::new();
                    for (tag, value_count, value_bytes, num_values) in tags {
                        let mut values = Vec::new();
                        if let Some(v) = value_count {
                            for _m in 0..(v as u32 * (num_values as u32)) {
                                let (value, length) =
                                    get_array_var_len(&mut self.reader, start, pos)?;

                                values.push(value);
                                pos += length as u64;
                            }
                        } else {
                            let mut count: usize = 0;
                            while count < value_bytes.unwrap() {
                                let (value, length) =
                                    get_array_var_len(&mut self.reader, start, pos)?;

                                values.push(value);
                                pos += length as u64;
                                count += length;
                            }
                        }
                        tag_map.insert(tag, values);
                    }
                    table.push((
                        cntx.get(tag_map.get(&3).and_then(|f| f.get(0)).unwrap_or(&0)),
                        tag_map,
                    ));
                }
            }

            let items = table.iter().enumerate().map(|(index, (name, map))| {
                return NCX::from(
                    index,
                    if let Some(n) = name {
                        n.to_string()
                    } else {
                        String::new()
                    },
                    &map,
                );
            });
        }
        Ok(())
    }

    /// 解析封面
    pub(crate) fn read_cover(&mut self) -> IResult<Option<Cover>> {
        if let Some(exth) = &self.exth_header {
            if let Some(offset) = exth.get_cover_offset().or(exth.get_thumbnail_offset()) {
                let (now, next) = self
                    .seek_record_offset(self.mobi_header.first_image_index + offset as u32)
                    .unwrap();

                let mut image = Vec::new();
                self.reader
                    .get_mut()
                    .take(next - now)
                    .read_to_end(&mut image)
                    .unwrap();
                return Ok(Some(Cover(image)));
            }
        }
        Ok(None)
    }

    /// 获取所有图片，由于是从文本中获取，所以可能不包括封面
    pub(crate) fn read_all_image(&mut self) -> IResult<Vec<MobiAssets>> {
        let text = self.read_text_raw()?;

        let index = read_image_recindex_from_html(text.as_slice())?;

        Ok(index
            .iter()
            .map(|f| {
                let (now, next) = self
                    .seek_record_offset(self.mobi_header.first_image_index + *f as u32)
                    .unwrap();

                let mut image = Vec::new();
                self.reader
                    .get_mut()
                    .take(next - now)
                    .read_to_end(&mut image)
                    .unwrap();

                return MobiAssets {
                    _file_name: format!("{}.{}", f, get_suffix(image.as_slice())),
                    media_type: String::new(),
                    _data: Some(image),
                    recindex: f.clone(),
                };
            })
            .collect())
    }

    /// 读取文本，注意这里并不将文本解码，依然保留原始字节
    pub(crate) fn read_text_raw(&mut self) -> IResult<Vec<u8>> {
        if let Some(v) = &self.text_cache {
            return Ok(v.clone());
        }
        // 获取所有text record
        let mut text: Vec<u8> = Vec::new();
        // let reader = &mut self.reader;
        let tail_circle_count = count_bit(self.mobi_header.extra_record_data_flags >> 1);

        // 第0个是header，所以从1开始
        for i in 1..(self.mobi_doc_header.record_count + 1) {
            let mut record: Vec<u8> = Vec::new();

            let (start, end) = self.seek_record_offset(i as u32)?;
            let len = end - start;
            // self.reader.seek(SeekFrom::Start(start))?;
            self.reader.get_mut().take(len).read_to_end(&mut record)?;

            // 处理尾巴
            let size = get_mobi_variable_width_len(
                &record,
                tail_circle_count,
                self.mobi_header.extra_record_data_flags,
            );

            for _ in 0..size {
                record.remove(record.len() - 1);
            }

            if self.mobi_doc_header.compression == 2 {
                // 解压缩
                record = uncompression_lz77(&record);
            }

            text.append(&mut record);
        }

        self.text_cache = Some(text.clone());

        return Ok(text);
    }

    /// 解码文本
    fn decode_text(&self, data: &[u8]) -> IResult<String> {
        if self.mobi_header.text_encoding == 1252 {
            // iSO-8859-1
            Ok(data.iter().map(|&c| c as char).collect())
        } else {
            String::from_utf8(data.to_vec()).map_err(|e| IError::Utf8(e))
        }
    }

    /// 加载文本，将文本分节，读取图片等信息
    pub(crate) fn load_text(&mut self) -> IResult<Vec<TextSection>> {
        let text = self.read_text_raw()?;

        // 查找子串
        let sub_bytes = b"<mbp:pagebreak/>";

        let mut i = 0;
        let mut prev = TextSection {
            index: 0,
            start: 0,
            end: 0,
            data: String::new(),
        };
        let mut pos = vec![];
        while i < text.len() {
            let mut now = &text[i];

            let mut j = 0;
            while j < sub_bytes.len() {
                if &sub_bytes[j] != now {
                    // i += j;
                    break;
                } else {
                    i += 1;
                    now = &text[i];
                }
                j += 1;
            }

            if j == sub_bytes.len() {
                // let prev = pos.last_mut().unwrap();

                prev.end = i - sub_bytes.len();

                prev.data = self.decode_text(
                    &text[(prev.start + if prev.start == 0 { 0 } else { sub_bytes.len() })
                        ..prev.end],
                )?;
                pos.push(prev);
                prev = TextSection {
                    index: pos.last().unwrap().index + 1,
                    start: pos.last().unwrap().end,
                    end: 0,
                    data: String::new(),
                }
                // pos.push(i - sub_bytes.len());
            }

            i += 1;
        }
        prev.end = i;
        prev.data = self.decode_text(&text[(prev.start + sub_bytes.len())..i])?;
        pos.push(prev);

        Ok(pos)
    }
}

/// 文本分节
#[derive(Debug, Default)]
pub(crate) struct TextSection {
    pub(crate) index: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    /// 被解码后可阅读的文本数据
    pub(crate) data: String,
}

///
/// [https://wiki.mobileread.com/wiki/MOBI#Variable-width_integers]
/// 看了好几遍，都还是没看懂文档是什么意思，只能把别的项目里的代码给翻译过来
///
fn get_mobi_variable_width_len(data: &[u8], tail_circle_count: usize, flag: u32) -> usize {
    let mut n_data = &data[..];
    for _ in 0..tail_circle_count {
        let res = buffer_get_var_len(n_data) as usize;
        n_data = &n_data[..(n_data.len() - res)];
    }

    if flag & 1 > 0 {
        let a = (n_data[n_data.len() - 1] & 0b11) + 1;
        n_data = &n_data[..(n_data.len() - a as usize)];
    }

    data.len() - n_data.len()
}

fn buffer_get_var_len(data: &[u8]) -> u32 {
    let array = &data[data.len() - 4..data.len()];
    let mut value: u32 = 0;
    for ele in array {
        if ele & 0b1000_0000 > 0 {
            value = 0;
        }
        let v: u32 = (*ele).into();
        value = (value << 7) | (v & 0b111_1111)
    }

    return value;
}

/// 解压缩
fn uncompression_lz77(data: &[u8]) -> Vec<u8> {
    let length = data.len();
    let mut offset = 0;
    let mut buffer = Vec::new();

    while offset < length {
        let char = data[offset];
        offset += 1;

        if char == 0 {
            buffer.push(char);
        } else if char <= 8 {
            for i in offset..(offset + char as usize) {
                buffer.push(data[i]);
            }
            offset += char as usize;
        } else if char <= 0x7f {
            buffer.push(char);
        } else if char <= 0xbf {
            let next = data[offset];
            offset += 1;
            let cc = char as usize;
            let distance = ((((cc << 8) | next as usize) >> 3) & 0x7ff) as usize;
            let lz_length = (next & 0x7) + 3;
            let mut buffer_size = buffer.len();

            for _ in 0..lz_length {
                buffer.push(buffer[buffer_size - distance]);
                buffer_size += 1;
            }
        } else {
            buffer.push(32);
            buffer.push(char ^ 0x80);
        }
    }

    buffer
}

#[cfg(test)]
mod tests {
    use std::io::Seek;

    use crate::mobi::{common::do_time_format, reader::is_mobi};

    use super::MobiReader;

    #[test]
    fn test_is_mobi() {
        let mut data: Vec<u8> = Vec::new();

        assert_eq!(
            false,
            is_mobi(&mut std::io::Cursor::new(&mut data)).unwrap()
        );

        let empty = [0u8; 60];
        data.append(&mut empty.to_vec());
        data.append(&mut b"BOOKMOB".to_vec());
        assert_eq!(
            false,
            is_mobi(&mut std::io::Cursor::new(&mut data)).unwrap()
        );

        data.append(&mut b"I".to_vec());
        assert_eq!(true, is_mobi(&mut std::io::Cursor::new(&mut data)).unwrap());

        for _ in 0..60 {
            data.push(0);
        }
        assert_eq!(true, is_mobi(&mut std::io::Cursor::new(&mut data)).unwrap());

        assert_eq!(
            false,
            is_mobi(&mut std::io::Cursor::new([0u8; 128])).unwrap()
        );
    }

    #[test]
    #[ignore = "only for dev"]
    fn test_header() {
        let path = std::env::current_dir().unwrap().join("demo.mobi");
        println!("dir {:?}", path);
        let fs = std::fs::File::open(path.to_str().unwrap()).unwrap();
        let mut h = MobiReader::new(fs).unwrap();

        println!("");

        println!("position = {}", h.reader.stream_position().unwrap());
        let exth = h.exth_header.as_ref().unwrap();
        println!(
            "exth  = {} {} {:?}",
            h.mobi_header.exth_flags,
            h.mobi_header.exth_flags & 64,
            exth
        );

        let book_info = h.read_meta_data().unwrap();

        println!("info = {:?}", book_info);

        println!(
            "{} {}",
            h.pdb_header.createion_date,
            do_time_format(h.pdb_header.createion_date)
        );

        println!("{}", do_time_format(3870581456));
        // println!("{}",
        // h.read_text().unwrap()
        // )
        // ;

        let sec = h.load_text().unwrap();
        println!("sec len = {}", sec.len());

        println!("{}", sec[1].data);
        println!("======");
        println!("{}", sec[46].data);

        let nav = h.read_nav_from_text(&sec[..]).unwrap().unwrap();

        println!("nav = {}", nav.len());

        // h.read_nav().unwrap();

        // h.read_cover()
        // .and_then(|op|op.ok_or(std::io::Error::new(std::io::ErrorKind::InvalidData, "no cover")))
        // .and_then(|mut cover| cover.write("cover1")).unwrap();

        // 尝试读取名字
    }

    // fn read_text(r: &mut MOBIReader) {
    //     let mut reader = &mut r.reader;

    //     for i in 0..r.mobi_doc_header.record_count {
    //         let mut v :Vec<u8> = vec![];
    //         reader.seek(pos)

    //     }
    // }
}
