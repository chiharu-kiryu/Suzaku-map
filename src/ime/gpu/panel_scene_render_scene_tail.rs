RenderScene {
    quads,
    text_quads,
    atlas_glyphs,
    text_sections,
    hit_targets,
    interactive_targets,
    sentence_candidate_truncated: sentence_candidate_truncated.clone(),
    next_token_candidate_truncated: next_token_candidate_truncated.clone(),
    handwriting_candidate_truncated: handwriting_candidate_truncated.clone(),
    labels: snapshot.candidate_labels.clone(),
    selected_label: snapshot
        .candidate_labels
        .get(snapshot.selected_index)
        .cloned(),
    draft_text: snapshot.draft_text.clone(),
}
