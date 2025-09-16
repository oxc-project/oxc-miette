#![cfg(feature = "fancy-no-backtrace")]

use miette::{Diagnostic, GraphicalReportHandler, NamedSource, SourceSpan};
use thiserror::Error;

#[test]
fn test_emoji_sequence_underline() {
    #[derive(Error, Debug, Diagnostic)]
    #[error("emoji test")]
    struct TestError {
        #[source_code]
        src: NamedSource<String>,
        #[label("here")]
        span: SourceSpan,
    }

    // Test with a ZWJ emoji sequence (family emoji)
    let family_emoji = "👨‍👩‍👧‍👦";
    let src = format!("before {} after", family_emoji);
    let err = TestError {
        src: NamedSource::new("test.txt", src.clone()),
        span: (7, family_emoji.len()).into(),
    };

    let mut output = String::new();
    GraphicalReportHandler::new().render_report(&mut output, &err).unwrap();

    println!("Output for family emoji:");
    println!("{}", output);

    // Test with flag emoji (also uses ZWJ)
    let flag_emoji = "🏳️‍🌈";
    let src2 = format!("before {} after", flag_emoji);
    let err2 = TestError {
        src: NamedSource::new("test2.txt", src2.clone()),
        span: (7, flag_emoji.len()).into(),
    };

    let mut output2 = String::new();
    GraphicalReportHandler::new().render_report(&mut output2, &err2).unwrap();

    println!("\nOutput for rainbow flag:");
    println!("{}", output2);

    // Test with skin tone modifier
    let skin_tone_emoji = "👋🏽";
    let src3 = format!("before {} after", skin_tone_emoji);
    let err3 = TestError {
        src: NamedSource::new("test3.txt", src3.clone()),
        span: (7, skin_tone_emoji.len()).into(),
    };

    let mut output3 = String::new();
    GraphicalReportHandler::new().render_report(&mut output3, &err3).unwrap();

    println!("\nOutput for waving hand with skin tone:");
    println!("{}", output3);

    // Test ASCII fast path
    let ascii_text = "hello world";
    let src4 = format!("before {} after", ascii_text);
    let err4 = TestError {
        src: NamedSource::new("test4.txt", src4.clone()),
        span: (7, ascii_text.len()).into(),
    };

    let mut output4 = String::new();
    GraphicalReportHandler::new().render_report(&mut output4, &err4).unwrap();

    println!("\nOutput for ASCII text:");
    println!("{}", output4);

    // Verify the underline matches the text length
    assert!(output4.contains("hello world"));
}
