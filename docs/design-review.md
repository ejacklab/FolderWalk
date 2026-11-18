# FolderWalk design review

ripgrep00 completed the independent read-only review in thread
01a1ed83-87de-7340-a742-e06c39bc40bb. The original reviewer, wuhu02,
reached an approval wait before producing findings. The replacement read
all four documents. Its outgoing reply reached the same approval wait, so
the lead recovered the written findings from the existing session record.
The tool later delivered the same findings directly.

| Finding | Resolution |
|---|---|
| The draft did not specify how to decode UTF-8, CSI, and bracketed paste markers across reads. A lone Escape timeout could incorrectly cancel an in-progress paste. | The design now specifies incremental decoding, complete paste framing, and a timeout limited to lone Escape outside a sequence or paste. The independent test brief will cover split markers and multibyte path text. |

The reviewer judged the filesystem interface, physical path semantics,
stable fixed-size viewport, and cleanup contract ready once this was clarified.
The review read the design and ran no implementation tests.
