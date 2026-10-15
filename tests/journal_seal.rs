use fjall::{Database, KeyspaceCreateOptions, PersistMode};

const ROWS: u32 = 1_000;

/// Writes `ROWS` items and flushes them, then seals the journal when `seal` is set.
fn write_and_flush(folder: &tempfile::TempDir, seal: bool) -> fjall::Result<()> {
    let db = Database::builder(folder).open()?;
    let items = db.keyspace("items", KeyspaceCreateOptions::default)?;
    for i in 0..ROWS {
        items.insert(i.to_be_bytes(), "value")?;
    }
    items.rotate_memtable_and_wait()?;
    if seal {
        db.seal_journal()?;
    }
    db.persist(PersistMode::SyncAll)
}

#[test_log::test]
fn a_reopen_without_a_seal_replays_the_active_journal() -> fjall::Result<()> {
    let folder = tempfile::tempdir()?;
    write_and_flush(&folder, false)?;

    let db = Database::builder(&folder).open()?;
    assert!(db.write_buffer_size() > 0);
    Ok(())
}

#[test_log::test]
fn a_reopen_after_flush_and_seal_replays_nothing() -> fjall::Result<()> {
    let folder = tempfile::tempdir()?;
    write_and_flush(&folder, true)?;

    let db = Database::builder(&folder).open()?;
    assert_eq!(0, db.write_buffer_size());
    assert_eq!(1, db.journal_count());

    let items = db.keyspace("items", KeyspaceCreateOptions::default)?;
    assert_eq!(ROWS as usize, items.len()?);
    Ok(())
}

#[test_log::test]
fn writes_after_the_seal_are_replayed() -> fjall::Result<()> {
    let folder = tempfile::tempdir()?;
    {
        let db = Database::builder(&folder).open()?;
        let items = db.keyspace("items", KeyspaceCreateOptions::default)?;
        items.insert("flushed", "value")?;
        items.rotate_memtable_and_wait()?;
        db.seal_journal()?;
        items.insert("late", "value")?;
        db.persist(PersistMode::SyncAll)?;
    }

    let db = Database::builder(&folder).open()?;
    assert!(db.write_buffer_size() > 0);

    let items = db.keyspace("items", KeyspaceCreateOptions::default)?;
    assert!(items.contains_key("flushed")?);
    assert!(items.contains_key("late")?);
    Ok(())
}
