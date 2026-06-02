use std::io::Cursor;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::categories::{self, AppleCategory};
use crate::error::{AppError, AppResult};
use crate::models::{Episode, Playlist};

/// Pre-computed values that depend on the *set* of episodes rather than
/// the playlist row itself. Resolved once before XML emission so we
/// don't repeat the same scan in every helper.
struct FeedContext<'a> {
    /// Apple-compatible `<itunes:category>` for the channel.
    /// Resolved from the first episode that has a category set.
    channel_category: Option<AppleCategory>,
    /// Image URL used as the channel's `<itunes:image>`.
    /// Resolution order: playlist's own cover → first episode that has a cover → None.
    channel_image: Option<&'a str>,
    /// True if the channel image came from the playlist itself (in which
    /// case we may reuse it as a per-episode fallback). False if it was
    /// inherited from an episode — in that case we don't propagate it
    /// further so we don't smear one episode's cover onto another.
    channel_image_is_playlist_own: bool,
    owner_email: Option<&'a str>,
}

pub fn build_feed(
    playlist: &Playlist,
    episodes: &[Episode],
    base_url: &str,
    owner_email: Option<&str>,
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

    let ctx = build_context(playlist, episodes, owner_email);

    write_channel(&mut writer, playlist, &ctx, base_url)?;

    for episode in episodes {
        write_episode(&mut writer, episode, &ctx)?;
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

fn build_context<'a>(
    playlist: &'a Playlist,
    episodes: &'a [Episode],
    owner_email: Option<&'a str>,
) -> FeedContext<'a> {
    let channel_category = episodes
        .iter()
        .find_map(|e| e.category_name.as_deref())
        .and_then(categories::resolve);

    let (channel_image, channel_image_is_playlist_own) = match playlist.cover_image_url.as_deref() {
        Some(url) => (Some(url), true),
        None => (
            episodes.iter().find_map(|e| e.cover_image_url.as_deref()),
            false,
        ),
    };

    FeedContext {
        channel_category,
        channel_image,
        channel_image_is_playlist_own,
        owner_email,
    }
}

fn write_channel<W: std::io::Write>(
    writer: &mut Writer<W>,
    playlist: &Playlist,
    ctx: &FeedContext,
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

    // <itunes:type>serial</itunes:type> — we number episodes via
    // <itunes:episode>1,2,3..., which is the serial signal Apple expects.
    write_text(writer, "itunes:type", "serial")?;

    write_text(writer, "itunes:explicit", "false")?;
    write_text(writer, "itunes:author", &playlist.owner_username)?;

    writer
        .write_event(Event::Start(BytesStart::new("itunes:owner")))
        .map_err(rss_err)?;
    write_text(writer, "itunes:name", &playlist.owner_username)?;
    if let Some(email) = ctx.owner_email {
        if !email.is_empty() {
            write_text(writer, "itunes:email", email)?;
        }
    }
    writer
        .write_event(Event::End(BytesEnd::new("itunes:owner")))
        .map_err(rss_err)?;

    if let Some(href) = ctx.channel_image {
        write_self_closing(writer, "itunes:image", &[("href", href)])?;
    }

    if let Some(cat) = &ctx.channel_category {
        write_itunes_category(writer, cat)?;
    }

    Ok(())
}

fn write_itunes_category<W: std::io::Write>(
    writer: &mut Writer<W>,
    cat: &AppleCategory,
) -> AppResult<()> {
    match cat.child {
        Some(child) => {
            let mut elem = BytesStart::new("itunes:category");
            elem.push_attribute(("text", cat.parent));
            writer.write_event(Event::Start(elem)).map_err(rss_err)?;
            write_self_closing(writer, "itunes:category", &[("text", child)])?;
            writer
                .write_event(Event::End(BytesEnd::new("itunes:category")))
                .map_err(rss_err)?;
        }
        None => {
            write_self_closing(writer, "itunes:category", &[("text", cat.parent)])?;
        }
    }
    Ok(())
}

fn write_episode<W: std::io::Write>(
    writer: &mut Writer<W>,
    episode: &Episode,
    ctx: &FeedContext,
) -> AppResult<()> {
    writer
        .write_event(Event::Start(BytesStart::new("item")))
        .map_err(rss_err)?;

    write_text(writer, "title", &episode.title)?;

    let description = episode.description.as_deref().unwrap_or("");
    write_text(writer, "description", description)?;
    write_cdata(writer, "content:encoded", description)?;

    write_guid(writer, &episode.id.to_string())?;

    write_text(writer, "itunes:episode", &episode.position.to_string())?;
    write_text(writer, "itunes:episodeType", "full")?;
    write_text(writer, "itunes:author", &episode.author_name)?;

    if let Some(duration) = episode.duration_seconds {
        write_text(writer, "itunes:duration", &duration.to_string())?;
    }

    let pub_date = episode.published_at.unwrap_or(episode.created_at);
    write_text(writer, "pubDate", &pub_date.to_rfc2822())?;

    // Episode image:
    //   1. episode's own cover, if any;
    //   2. else playlist's own cover (but NOT an inherited channel image
    //      that we picked from a sibling episode — we don't want to
    //      smear one episode's art onto another).
    let episode_image = episode.cover_image_url.as_deref().or_else(|| {
        if ctx.channel_image_is_playlist_own {
            ctx.channel_image
        } else {
            None
        }
    });
    if let Some(href) = episode_image {
        write_self_closing(writer, "itunes:image", &[("href", href)])?;
    }

    // RSS clients can't consume the HLS playlist that lives in
    // podcasts.audio_url, so <enclosure> uses the direct file URL.
    // SQL filter already excludes rows where audio_url_file IS NULL.
    let enclosure_url = episode.audio_url_file.as_str();
    let mime = guess_audio_mime(enclosure_url);
    let length = episode.audio_size_bytes.unwrap_or(0).to_string();
    write_self_closing(
        writer,
        "enclosure",
        &[
            ("url", enclosure_url),
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

/// `<guid>` carries the bare episode UUID, which is not a URL. RSS 2.0
/// treats a guid as a permalink unless `isPermaLink="false"` is set, so we
/// emit it explicitly to stop clients from trying to resolve the UUID.
fn write_guid<W: std::io::Write>(writer: &mut Writer<W>, guid: &str) -> AppResult<()> {
    let mut start = BytesStart::new("guid");
    start.push_attribute(("isPermaLink", "false"));
    writer.write_event(Event::Start(start)).map_err(rss_err)?;
    writer
        .write_event(Event::Text(BytesText::new(guid)))
        .map_err(rss_err)?;
    writer
        .write_event(Event::End(BytesEnd::new("guid")))
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
