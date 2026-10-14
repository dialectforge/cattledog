# Rust crates (from ai-factory)

*Created by John Dean Martin.*


## Open-source release notes

> **License:** Apache-2.0; see the License section below.
>
> Internal notes, AI-assistant files, build artifacts and personal machine paths were removed for this release. Never commit `.env`, keys or signing files (see `.gitignore`).

> **Paths:** scripts under `tools/`, `v2/tools/` and `overnight_train_v4.py` use the placeholders `<AI_FACTORY_ROOT>` (checkout directory) and `<USER_HOME>` (your home folder). Replace them before running. The Rust crates now use paths relative to the working directory (`brain`, `lu_debug.log`) instead of a hardcoded drive path.

## License

Copyright 2026 DialectForge LLC. Licensed under the Apache License, Version 2.0; see [LICENSE](../LICENSE). Source files carry an `SPDX-License-Identifier: Apache-2.0` header.

## Semantic embeddings (BGE-small)

The v2 `embedding` crate can use the **BGE-small-en-v1.5** sentence-embedding model (384 dimensions) through ONNX Runtime (`fastembed`/`ort`). The model weights are **not** included in this repository. To enable it:

1. Download the ONNX export of [`BAAI/bge-small-en-v1.5`](https://huggingface.co/BAAI/bge-small-en-v1.5) (for example `Xenova/bge-small-en-v1.5` or `Qdrant/bge-small-en-v1.5-onnx-Q`), e.g. `huggingface-cli download Qdrant/bge-small-en-v1.5-onnx-Q --local-dir v2/models/bge-small-en-v1.5`.
2. Make sure that folder contains the ONNX model plus `tokenizer.json`, `config.json`, `special_tokens_map.json` and `tokenizer_config.json`.
3. Point `LU_BGE_DIR` at that folder (it defaults to `v2/models/bge-small-en-v1.5` relative to the working directory). If the model is missing, Lu falls back to the lexical embedder.

`v2/models/` is git-ignored, so don't commit model weights.
