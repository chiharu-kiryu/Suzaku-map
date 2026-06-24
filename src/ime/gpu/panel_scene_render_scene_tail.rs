RenderScene {
    quads,
    text_quads,
    atlas_glyphs,
    text_sections,
    hit_targets,
    interactive_targets,
    labels: snapshot.candidate_labels.clone(),
    selected_label: snapshot
        .candidate_labels
        .get(snapshot.selected_index)
        .cloned(),
    draft_text: snapshot.draft_text.clone(),
}
