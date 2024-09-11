/// pdb中的一个record info
/// 一组8个字节
#[derive(Default, Debug,Clone)]
pub(crate) struct PDBRecordInfo {
    ///  the offset of record n from the start of the PDB of this record
    pub(crate) offset: u32,
    /// bit field. The least significant four bits are used to represent the category values. These are the categories used to split the databases for viewing on the screen. A few of the 16 categories are pre-defined but the user can add their own. There is an undefined category for use if the user or programmer hasn't set this.
    /// 0x10 (16 decimal) Secret record bit.
    /// 0x20 (32 decimal) Record in use (busy bit).
    /// 0x40 (64 decimal) Dirty record bit.
    /// 0x80 (128, unsigned decimal) Delete record on next HotSync.
    pub(crate) attribute: u8,
    /// The unique ID for this record. Often just a sequential count from 0
    /// 实际是只有3个字节，最高位的一个字节不使用
    pub(crate) unique_id: u32,
}

#[derive(Default, Debug)]
pub(crate) struct PDBHeader {
    // name(32)
    pub(crate) name: [u8; 32],
    // attribute(2)
    ///
    /// 0x0002 Read-Only
    /// 0x0004 Dirty AppInfoArea
    /// 0x0008 Backup this database (i.e. no conduit exists)
    /// 0x0010 (16 decimal) Okay to install newer over existing copy, if present on PalmPilot
    /// 0x0020 (32 decimal) Force the PalmPilot to reset after this database is installed
    /// 0x0040 (64 decimal) Don't allow copy of file to be beamed to other Pilot.
    ///
    pub(crate) attribute: u16,
    /// file version
    pub(crate) version: u16,
    /// No. of seconds since start of January 1, 1904.
    ///
    /// [https://wiki.mobileread.com/wiki/PDB#PDB%20Times] 对于时间又有新的规定
    ///
    /// If the time has the top bit set, it's an unsigned 32-bit number counting from 1st Jan 1904
    ///
    /// If the time has the top bit clear, it's a signed 32-bit number counting from 1st Jan 1970.
    ///
    pub(crate) createion_date: u32,
    /// No. of seconds since start of January 1, 1904.
    pub(crate) modify_date: u32,
    /// No. of seconds since start of January 1, 1904.
    pub(crate) last_backup_date: u32,
    /// No. of seconds since start of January 1, 1904.
    pub(crate) modification_number: u32,
    /// offset to start of Application Info (if present) or null
    pub(crate) app_info_id: u32,
    /// offset to start of Sort Info (if present) or null
    pub(crate) sort_info_id: u32,
    /// See above table. (For Applications this data will be 'appl')
    pub(crate) _type: [u8; 4],
    /// See above table. This program will be launched if the file is tapped
    pub(crate) creator: [u8; 4],
    /// used internally to identify record
    pub(crate) unique_id_seed: u32,
    /// Only used when in-memory on Palm OS. Always set to zero in stored files.
    pub(crate) next_record_list_id: u32,
    /// number of records in the file - N
    pub(crate) number_of_records: u16,
    /// record，每个8个字节，所有list结束后，有两个字节的空隙，无实际意义
    pub(crate) record_info_list: Vec<PDBRecordInfo>,
}
#[derive(Default, Debug)]
pub(crate) struct MOBIDOCHeader {
    ///  1 == no compression, 2 = PalmDOC compression, 17480 = HUFF/CDIC compression
    /// 之后跳过2字节无用
    pub(crate) compression: u16,
    /// Uncompressed length of the entire text of the book
    pub(crate) length: u32,
    /// Number of PDB records used for the text of the book.
    pub(crate) record_count: u16,
    /// Maximum size of each record containing text, always 4096
    pub(crate) record_size: u16,
    /// Current reading position, as an offset into the uncompressed text
    /// 如果 compression = 17480  ，这个字段会被拆分开
    pub(crate) position: u32,
    /// compression = 17480 时才有该字段
    ///   0 == no encryption, 1 = Old Mobipocket Encryption, 2 = Mobipocket Encryption
    pub(crate) encrypt_type: u16,
}
#[derive(Default, Debug)]
pub(crate) struct MOBIHeader {
    // the characters M O B I
    ///  the length of the MOBI header, including the previous 4 bytes
    pub(crate) header_len: u32,
    /// The kind of Mobipocket file this is
    /// 2 Mobipocket Book
    /// 3 PalmDoc Book
    /// 4 Audio
    /// 232 mobipocket? generated by kindlegen1.2
    /// 248 KF8: generated by kindlegen2
    /// 257 News
    /// 258 News_Feed
    /// 259 News_Magazine
    /// 513 PICS
    /// 514 WORD
    /// 515 XLS
    /// 516 PPT
    /// 517 TEXT
    /// 518 HTML
    pub(crate) mobi_type: u32,
    /// 1252 = CP1252 (WinLatin1); 65001 = UTF-8
    pub(crate) text_encoding: u32,
    /// Some kind of unique ID number (random?)
    pub(crate) unique_id: u32,
    /// Version of the Mobipocket format used in this file.
    pub(crate) file_version: u32,
    /// Section number of orthographic meta index. 0xFFFFFFFF if index is not available.
    pub(crate) ortographic_index: u32,
    /// Section number of inflection meta index. 0xFFFFFFFF if index is not available.
    pub(crate) inflection_index: u32,
    /// 0xFFFFFFFF if index is not available.
    pub(crate) index_names: u32,
    /// 0xFFFFFFFF if index is not available.
    pub(crate) index_keys: u32,
    /// Section number of extra N meta index. 0xFFFFFFFF if index is not available.
    pub(crate) extra_index: [u32; 6],
    /// First record number (starting with 0) that's not the book's text
    pub(crate) first_non_book_index: u32,
    /// Offset in record 0 (not from start of file) of the full name of the book
    pub(crate) full_name_offset: u32,

    ///  Length in bytes of the full name of the book
    pub(crate) full_name_length: u32,
    ///  Book locale code. Low byte is main language 09= English, next byte is dialect, 08 = British, 04 = US. Thus US English is 1033, UK English is 2057.
    pub(crate) locale: u32,
    /// Input language for a dictionary
    pub(crate) input_language: u32,
    /// Output language for a dictionary
    pub(crate) output_language: u32,
    /// Minimum mobipocket version support needed to read this file.
    pub(crate) min_version: u32,
    /// First record number (starting with 0) that contains an image. Image records should be sequential.
    pub(crate) first_image_index: u32,
    /// The record number of the first huffman compression record.
    pub(crate) huffman_record_offset: u32,
    /// The number of huffman compression records.
    pub(crate) huffman_record_count: u32,
    ///     
    pub(crate) huffman_table_offset: u32,
    ///     
    pub(crate) huffman_table_length: u32,
    /// bitfield. if bit 6 (0x40) is set, then there's an EXTH record
    /// 当从低到高第六位为1，代表有EXTH，与其他bit无关
    pub(crate) exth_flags: u32,
    // 32 unknown bytes, if MOBI is long enough
    // unknown_0: [u8; 8],
    // /// Use 0xFFFFFFFF
    // unknown_1: u32,
    /// Offset to DRM key info in DRMed files. 0xFFFFFFFF if no DRM
    /// 实际 没有drm这里是0？待测试
    pub(crate) drm_offset: u32,
    /// Number of entries in DRM info. 0xFFFFFFFF if no DRM
    pub(crate) drm_count: u32,
    /// Number of bytes in DRM info.
    pub(crate) drm_size: u32,
    /// Some flags concerning the DRM info.
    pub(crate) drm_flags: u32,
    // Bytes to the end of the MOBI header, including the following if the header length >= 228 (244 from start of record).Use 0x0000000000000000.
    // unknown_2: u64,

    /// Number of first text record. Normally 1.
    pub(crate) first_content_record_number: u16,
    /// Number of last image record or number of last text record if it contains no images. Includes Image, DATP, HUFF, DRM.
    pub(crate) last_content_record_number: u16,
    // FCIS record count? Use 0x00000001.
    // unknown_3: u32,
    ///
    pub(crate) fcis_record_number: u32,
    // Use 0x00000001.
    // unknown_4: u32,
    ///
    pub(crate) flis_record_number: u32,
    // Use 0x00000001.flis record count?
    // unknown_5: u32,
    // Use 0x0000000000000000.
    // unknown_6: u64,
    // Use 0xFFFFFFFF.
    // unknown_7: u32,
    /// Use 0x00000000.
    pub(crate) first_compilation_data_section_count: u32,
    /// Use 0xFFFFFFFF.
    pub(crate) number_of_compilation_data_sections: u32,
    // Use 0xFFFFFFFF.
    // unknown_8: u32,
    /// A set of binary flags, some of which indicate extra data at the end of each text block. This only seems to be valid for Mobipocket format version 5 and 6 (and higher?), when the header length is 228 (0xE4) or 232 (0xE8).
    /// bit 1 (0x1) : <extra multibyte bytes><size>
    /// bit 2 (0x2) : <TBS indexing description of this HTML record><size>
    /// bit 3 (0x4) : <uncrossable breaks><size>
    /// Setting bit 2 (0x2) disables <guide><reference type="start"> functionality.
    pub(crate) extra_record_data_flags: u32,
    /// (If not 0xFFFFFFFF)The record number of the first INDX record created from an ncx file.
    pub(crate) indx_record_offset: u32,

}

#[derive(Default, Debug)]
pub(crate) struct EXTHRecord {
    /// Exth Record type. Just a number identifying what's stored in the record
    pub(crate) _type: u32,
    /// length of EXTH record = L , including the 8 bytes in the type and length fields
    pub(crate) len: u32,
    /// Data，L - 8
    pub(crate) data: Vec<u8>,
}
///
///
/// 参见 [https://wiki.mobileread.com/wiki/MOBI#EXTH_Header]
///
#[derive(Default, Debug)]
pub(crate) struct EXTHHeader {
    // the characters E X T H
    // identifier: [u8; 4],
    /// the length of the EXTH header, including the previous 4 bytes - but not including the final padding.
    pub(crate) len: u32,
    /// The number of records in the EXTH header. the rest of the EXTH header consists of repeated EXTH records to the end of the EXTH length.
    pub(crate) record_count: u32,
    /// 不定长度的 record,
    pub(crate) record_list: Vec<EXTHRecord>, // 多余的字节均为无用填充，跳过即可
}
#[derive(Debug, Default)]
pub(crate) struct INDXRecord {
    /// 在之前还有 4个字节的 Identifier，固定为I N D X
    /// the length of the INDX header, including the previous 4 bytes
    pub(crate) len: u32,
    pub(crate) _type: u32,
    /// 前面还有8个无用字节
    /// the offset to the IDXT section
    pub(crate) idxt_start: u32,
    /// the number of index records
    pub(crate) index_count: u32,
    /// 1252 = CP1252 (WinLatin1); 65001 = UTF-8
    pub(crate) index_encoding: u32,
    /// the language code of the index
    pub(crate) index_language: u32,
    /// the number of index entries
    pub(crate) total_index_count: u32,
    /// the offset to the ORDT section
    pub(crate) ordt_start: u32,
    /// the offset to the LIGT section
    pub(crate) ligt_start: u32,
    /// 文档没有描述
    pub(crate) ligt_count: u32,
    /// 文档没有描述
    pub(crate) cncx_count: u32,
}
/// 格式化时间戳
pub(crate) fn do_time_format(value: u32) -> String {
    if value & 0x80000000 == 0x80000000 {
        crate::common::do_time_display((value & 0x7fffffff) as u64, 1904)
    } else {
        crate::common::time_display(value as u64)
    }
}
fn u8_to_string<const N: usize>(v: [u8; N]) -> String {
    // let mut v = [0u8;4];
    // v[0] = (value >> 24 & 0xff) as u8;
    // v[1]=(value >> 16 & 0xff) as u8;
    // v[2] = (value >> 8 & 0xff) as u8;
    // v[3] = (value & 0xff) as u8;

    String::from_utf8(v.to_vec()).unwrap_or(String::new())
}
impl std::fmt::Display for PDBHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f,"PDBHeader {{ name: '{}', attribute: {}, version: {}, createion_date: {}, modify_date: {}, last_backup_date: {}, modification_number: {}, app_info_id: {}, sort_info_id: {}, _type: {}, creator: {}, unique_id_seed: {}, next_record_list_id: {}, number_of_records: {}, record_info_list: {:?}, record_list: [] }}"
            ,u8_to_string(self.name)
        ,self.attribute
        ,self.version
        ,do_time_format(self.createion_date)
        ,do_time_format(self.modify_date)
        ,do_time_format(self.last_backup_date)
        ,self.modification_number
        ,self.app_info_id
        ,self.sort_info_id
        ,u8_to_string(self._type)
        ,u8_to_string(self.creator)
        ,self.unique_id_seed
        ,self.next_record_list_id
        ,self.number_of_records
        ,self.record_info_list

            )
    }
}
#[derive(Debug)]
pub(crate) struct NCX {
    pub(crate) index: usize,
    pub(crate) offset: Option<usize>,
    pub(crate) size: Option<usize>,
    pub(crate) label: String,
    pub(crate) heading_lebel: usize,
    pub(crate) pos: usize,
    pub(crate) parent: Option<usize>,
    pub(crate) first_child: Option<usize>,
    pub(crate) last_child: Option<usize>,
}