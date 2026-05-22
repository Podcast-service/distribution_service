use std::io::Cursor;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::error::{AppError, AppResult};
use crate::models::{Episode, Playlist};

pub fn build_feed(
    playlist: &Playlist,
    episodes: &[Episode],
    base_url: &str,
) -> AppResult<String> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::<u8>::new()), b' ', 2);

    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(rss_err)?;

    let mut rss = BytesStart::new("rss");
    rss.push_attribute(("version", "2.0"));
    rss.push_attribute(("xmlns:itunes", "http://www.itunes.com/dtds/podcast-1.0.dtd"));
    rss.push_attribute(("xmlns:content", "http://purl.org/rss/1.0/modules/content/"));
    writer.write_event(Event::Start(rss)).map_err(rss_err)?;

    writer
        .write_event(Event::Start(BytesStart::new("channel")))
        .map_err(rss_err)?;

    write_channel(&mut writer, playlist, base_url)?;

    for episode in episodes {
        write_episode(&mut writer, episode, base_url)?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("channel")))
        .map_err(rss_err)?;
    writer
        .write_event(Event::End(BytesEnd::new("rss")))
        .map_err(rss_err)?;

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).map_err(|e| AppError::Rss(e.to_string()))
}

fn write_channel<W: std::io::Write>(
    writer: &mut Writer<W>,
    playlist: &Playlist,
    base_url: &str,
) -> AppResult<()> {
    write_text(writer, "title", &playlist.title)?;

    let link = format!("{}/feed/{}.xml", base_url.trim_end_matches('/'), playlist.id);
    write_text(writer, "link", &link)?;

    write_text(
        writer,
        "description",
        playlist.description.as_deref().unwrap_or(""),
    )?;

    let language = map_language(&playlist.owner_language);
    write_text(writer, "language", language)?;

    write_text(writer, "generator", "distribution_service")?;

    write_text(writer, "pubDate", &playlist.updated_at.to_rfc2822())?;
    write_text(writer, "lastBuildDate", &playlist.updated_at.to_rfc2822())?;

    write_text(writer, "itunes:explicit", "false")?;
    write_text(writer, "itunes:author", &playlist.owner_username)?;

    writer
        .write_event(Event::Start(BytesStart::new("itunes:owner")))
        .map_err(rss_err)?;
    write_text(writer, "itunes:name", &playlist.owner_username)?;
    writer
        .write_event(Event::End(BytesEnd::new("itunes:owner")))
        .map_err(rss_err)?;

    let channel_image = playlist
        .cover_image_url
        .as_deref()
        .or(playlist.owner_avatar_url.as_deref());
    if let Some(href) = channel_image {
        write_self_closing(writer, "itunes:image", &[("href", href)])?;
    }

    Ok(())
}

fn write_episode<W: std::io::Write>(
    writer: &mut Writer<W>,
    episode: &Episode,
    _base_url: &str,
) -> AppResult<()> {
    writer
        .write_event(Event::Start(BytesStart::new("item")))
        .map_err(rss_err)?;

    write_text(writer, "title", &episode.title)?;

    let description = episode.description.as_deref().unwrap_or("");
    write_text(writer, "description", description)?;
    write_cdata(writer, "content:encoded", description)?;

    write_text(writer, "guid", &episode.id.to_string())?;

    write_text(writer, "itunes:episode", &episode.position.to_string())?;
    write_text(writer, "itunes:episodeType", "full")?;
    write_text(writer, "itunes:author", &episode.author_name)?;

    if let Some(duration) = episode.duration_seconds {
        write_text(writer, "itunes:duration", &duration.to_string())?;
    }

    let pub_date = episode.published_at.unwrap_or(episode.created_at);
    write_text(writer, "pubDate", &pub_date.to_rfc2822())?;

    if let Some(cover) = &episode.cover_image_url {
        write_self_closing(writer, "itunes:image", &[("href", cover)])?;
    }

    let mime = guess_audio_mime(&episode.audio_url);
    let length = episode.audio_size_bytes.unwrap_or(0).to_string();
    write_self_closing(
        writer,
        "enclosure",
        &[
            ("url", episode.audio_url.as_str()),
            ("length", length.as_str()),
            ("type", mime),
        ],
    )?;

    writer
        .write_event(Event::End(BytesEnd::new("item")))
        .map_err(rss_err)?;

    Ok(())
}

fn write_text<W: std::io::Write>(
    writer: &mut Writer<W>,
    tag: &str,
    text: &str,
) -> AppResult<()> {
    writer
        .write_event(Event::Start(BytesStart::new(tag)))
        .map_err(rss_err)?;
    writer
        .write_event(Event::Text(BytesText::new(text)))
        .map_err(rss_err)?;
    writer
        .write_event(Event::End(BytesEnd::new(tag)))
        .map_err(rss_err)?;
    Ok(())
}

fn write_cdata<W: std::io::Write>(
    writer: &mut Writer<W>,
    tag: &str,
    text: &str,
) -> AppResult<()> {
    use quick_xml::events::BytesCData;

    writer
        .write_event(Event::Start(BytesStart::new(tag)))
        .map_err(rss_err)?;
    writer
        .write_event(Event::CData(BytesCData::new(text)))
        .map_err(rss_err)?;
    writer
        .write_event(Event::End(BytesEnd::new(tag)))
        .map_err(rss_err)?;
    Ok(())
}

fn write_self_closing<W: std::io::Write>(
    writer: &mut Writer<W>,
    tag: &str,
    attrs: &[(&str, &str)],
) -> AppResult<()> {
    let mut elem = BytesStart::new(tag);
    for (k, v) in attrs {
        elem.push_attribute((*k, *v));
    }
    writer.write_event(Event::Empty(elem)).map_err(rss_err)?;
    Ok(())
}

fn rss_err(e: quick_xml::Error) -> AppError {
    AppError::Rss(e.to_string())
}

fn map_language(db_lang: &str) -> &'static str {
    match db_lang.to_ascii_uppercase().as_str() {
        "RU" => "ru",
        "EN" => "en",
        _ => "en",
    }
}

fn guess_audio_mime(url: &str) -> &'static str {
    let lower = url.to_ascii_lowercase();
    let path = lower.split('?').next().unwrap_or(&lower);
    if path.ends_with(".mp3") {
        "audio/mpeg"
    } else if path.ends_with(".m4a") || path.ends_with(".aac") {
        "audio/mp4"
    } else if path.ends_with(".ogg") || path.ends_with(".oga") {
        "audio/ogg"
    } else if path.ends_with(".opus") {
        "audio/opus"
    } else if path.ends_with(".wav") {
        "audio/wav"
    } else if path.ends_with(".flac") {
        "audio/flac"
    } else if path.ends_with(".m3u8") {
        "application/vnd.apple.mpegurl"
    } else {
        "audio/mpeg"
    }
}
