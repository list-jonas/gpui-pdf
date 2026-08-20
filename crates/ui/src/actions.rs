use gpui::actions;

actions!(
    pdf_editor,
    [
        OpenDocument,
        SaveDocument,
        NextPage,
        PreviousPage,
        SelectTool,
        HandTool,
        HighlightTool,
        AddTextTool,
        RedactTool,
        ZoomIn,
        ZoomOut,
        ActualSize,
        FitPage,
        CommitText,
        CopySelection,
        Search,
        NextSearchResult,
        PreviousSearchResult
    ]
);
