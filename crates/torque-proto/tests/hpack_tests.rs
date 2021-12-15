use torque_proto::http2::hpack::huffman::{huffman_decode, huffman_encode};
use torque_proto::http2::hpack::{DynamicTable, HeaderTable, HpackDecoder, HpackEncoder};

#[test]
fn test_static_table_entries() {
    let table = HeaderTable::new(4096);
    let entry1 = table.get(2).expect("Index 2 must exist");
    assert_eq!(entry1.name, ":method");
    assert_eq!(entry1.value, "GET");

    let entry4 = table.get(4).expect("Index 4 must exist");
    assert_eq!(entry4.name, ":path");
    assert_eq!(entry4.value, "/");
}

#[test]
fn test_dynamic_table_eviction() {
    let mut table = DynamicTable::new(128); // small capacity
    table.insert("custom-key-1".to_string(), "val-1".to_string());
    assert_eq!(table.len(), 1);

    table.insert("custom-key-2".to_string(), "val-2".to_string());
    assert_eq!(table.len(), 2);

    // Insert large header that forces eviction
    table.insert("huge-header-name".to_string(), "very-long-header-value-overflowing-buffer".to_string());
    assert!(table.len() <= 2);
}

#[test]
fn test_huffman_roundtrip() {
    let original = b"www.example.com/api/v2/metrics?token=xyz123";
    let mut encoded = Vec::new();
    huffman_encode(original, &mut encoded);

    // Huffman encoded output must be smaller than plain ASCII
    assert!(encoded.len() <= original.len());

    let decoded = huffman_decode(&encoded).expect("huffman decode failed");
    assert_eq!(&decoded[..], original);
}

#[test]
fn test_hpack_encode_decode_roundtrip() {
    let mut encoder = HpackEncoder::new();
    let mut decoder = HpackDecoder::new();

    let headers = vec![
        (":method", "POST"),
        (":path", "/v1/orders"),
        (":scheme", "https"),
        ("content-type", "application/json"),
        ("x-custom-request-id", "req-abcdef-987654321"),
    ];

    let mut encoded_block = Vec::new();
    encoder.encode(headers.clone(), &mut encoded_block);

    let decoded_fields = decoder.decode(&encoded_block).expect("hpack decode failed");
    assert_eq!(decoded_fields.len(), headers.len());

    for (i, (expected_name, expected_val)) in headers.iter().enumerate() {
        assert_eq!(&decoded_fields[i].name, expected_name);
        assert_eq!(&decoded_fields[i].value, expected_val);
    }
}
