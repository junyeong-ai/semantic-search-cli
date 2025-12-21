# Semantic Search CLI

[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![Release](https://img.shields.io/github/v/release/junyeong-ai/semantic-search-cli?style=flat-square&logo=github)](https://github.com/junyeong-ai/semantic-search-cli/releases/latest)
[![DeepWiki](https://img.shields.io/badge/DeepWiki-junyeong--ai%2Fsemantic--search--cli-blue.svg?logo=data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACwAAAAyCAYAAAAnWDnqAAAAAXNSR0IArs4c6QAAA05JREFUaEPtmUtyEzEQhtWTQyQLHNak2AB7ZnyXZMEjXMGeK/AIi+QuHrMnbChYY7MIh8g01fJoopFb0uhhEqqcbWTp06/uv1saEDv4O3n3dV60RfP947Mm9/SQc0ICFQgzfc4CYZoTPAswgSJCCUJUnAAoRHOAUOcATwbmVLWdGoH//PB8mnKqScAhsD0kYP3j/Yt5LPQe2KvcXmGvRHcDnpxfL2zOYJ1mFwrryWTz0advv1Ut4CJgf5uhDuDj5eUcAUoahrdY/56ebRWeraTjMt/00Sh3UDtjgHtQNHwcRGOC98BJEAEymycmYcWwOprTgcB6VZ5JK5TAJ+fXGLBm3FDAmn6oPPjR4rKCAoJCal2eAiQp2x0vxTPB3ALO2CRkwmDy5WohzBDwSEFKRwPbknEggCPB/imwrycgxX2NzoMCHhPkDwqYMr9tRcP5qNrMZHkVnOjRMWwLCcr8ohBVb1OMjxLwGCvjTikrsBOiA6fNyCrm8V1rP93iVPpwaE+gO0SsWmPiXB+jikdf6SizrT5qKasx5j8ABbHpFTx+vFXp9EnYQmLx02h1QTTrl6eDqxLnGjporxl3NL3agEvXdT0WmEost648sQOYAeJS9Q7bfUVoMGnjo4AZdUMQku50McDcMWcBPvr0SzbTAFDfvJqwLzgxwATnCgnp4wDl6Aa+Ax283gghmj+vj7feE2KBBRMW3FzOpLOADl0Isb5587h/U4gGvkt5v60Z1VLG8BhYjbzRwyQZemwAd6cCR5/XFWLYZRIMpX39AR0tjaGGiGzLVyhse5C9RKC6ai42ppWPKiBagOvaYk8lO7DajerabOZP46Lby5wKjw1HCRx7p9sVMOWGzb/vA1hwiWc6jm3MvQDTogQkiqIhJV0nBQBTU+3okKCFDy9WwferkHjtxib7t3xIUQtHxnIwtx4mpg26/HfwVNVDb4oI9RHmx5WGelRVlrtiw43zboCLaxv46AZeB3IlTkwouebTr1y2NjSpHz68WNFjHvupy3q8TFn3Hos2IAk4Ju5dCo8B3wP7VPr/FGaKiG+T+v+TQqIrOqMTL1VdWV1DdmcbO8KXBz6esmYWYKPwDL5b5FA1a0hwapHiom0r/cKaoqr+27/XcrS5UwSMbQAAAABJRU5ErkJggg==)](https://deepwiki.com/junyeong-ai/semantic-search-cli)

> **[English](README.en.md)** | **한국어**

**터미널에서 의미 기반 검색.** 로컬 코드, Jira, Confluence, Figma를 하나의 명령어로 검색합니다.

---

## 왜 Semantic Search CLI인가?

- **의미 검색** — 키워드가 아닌 의미로 검색 (Qwen3 1024차원 임베딩)
- **통합 검색** — 로컬 파일 + Jira + Confluence + Figma
- **자동화** — ML 데몬 자동 시작, Claude Code 통합

---

## 빠른 시작

```bash
# 설치
git clone https://github.com/junyeong-ai/semantic-search-cli && cd semantic-search-cli
./scripts/install.sh

# Qdrant 시작
docker-compose up -d qdrant

# 인덱싱 & 검색
ssearch index add ./src
ssearch search "사용자 인증 로직"
```

---

## 주요 기능

### 검색
```bash
ssearch search "API 설계"                      # 기본 검색
ssearch search "결제" --source jira            # Jira만
ssearch search "에러" --tags "project:main"    # 태그 필터
ssearch search "인증" --min-score 0.7          # 유사도 필터
ssearch search "설계" --format json            # JSON 출력
```

### 인덱싱
```bash
ssearch index add ./src                        # 디렉토리
ssearch index add . --tags "project:myapp"     # 태그 추가
ssearch index add . -e "node_modules" -e ".git" # 제외 패턴
ssearch index delete ./old                     # 삭제
ssearch index clear -y                         # 전체 삭제
```

### 외부 소스 동기화
```bash
# Jira
ssearch source sync jira --project MYPROJ --all        # 프로젝트 전체 (스트리밍)
ssearch source sync jira --project MYPROJ --limit 100  # 배치 모드
ssearch source sync jira --query "PROJ-1234"           # 단일 이슈

# Confluence
ssearch source sync confluence --project DOCS --all    # 스페이스 전체
ssearch source sync confluence --query "12345678"      # 단일 페이지

# Figma
ssearch source sync figma --query "https://figma.com/design/xxx?node-id=123"
```

### 관리
```bash
ssearch status              # 인프라 상태
ssearch tags list           # 태그 목록
ssearch source list         # 소스 목록
ssearch serve restart       # ML 데몬 재시작
```

---

## 설치

### 자동 설치 (권장)
```bash
git clone https://github.com/junyeong-ai/semantic-search-cli && cd semantic-search-cli
./scripts/install.sh
```

### 수동 빌드
```bash
cargo build --release
cp target/release/ssearch ~/.local/bin/
```

**요구사항**: Docker (Qdrant용)

---

## 설정

### 설정 파일 (우선순위 순서)
1. 환경변수 (`SSEARCH_*`)
2. 프로젝트 설정 (`.ssearch/config.toml`)
3. 전역 설정 (`~/.config/ssearch/config.toml`)

전역 설정 예시:

```toml
[embedding]
model_id = "JunyeongAI/qwen3-embedding-0.6b-onnx"
dimension = 1024
batch_size = 8

[vector_store]
driver = "qdrant"           # qdrant | postgresql
url = "http://localhost:16334"
collection = "semantic_search"

[indexing]
chunk_size = 6000
chunk_overlap = 500
max_file_size = 10485760    # 10MB

[search]
default_limit = 10
default_format = "text"     # text | json | markdown

[daemon]
idle_timeout_secs = 600     # 10분 후 자동 종료
auto_start = true

[metrics]
enabled = true
retention_days = 30
```

---

## 명령어 참조

| 명령어 | 설명 |
|--------|------|
| `search <query>` | 시맨틱 검색 |
| `index add <path>` | 파일 인덱싱 |
| `index delete <path>` | 삭제 |
| `index clear` | 전체 삭제 |
| `source sync <type>` | 외부 소스 동기화 |
| `source list` | 소스 목록 |
| `source delete <type>` | 소스별 삭제 |
| `tags list` | 태그 목록 |
| `tags delete <tag>` | 태그별 삭제 |
| `import <file>` | JSON/JSONL 가져오기 |
| `status` | 상태 확인 |
| `serve restart` | 데몬 재시작 |
| `config init/show/edit` | 설정 관리 |

### 검색 옵션

| 옵션 | 설명 |
|------|------|
| `-n, --limit` | 결과 수 (기본: 10) |
| `-t, --tags` | 태그 필터 (`key:value`) |
| `-s, --source` | 소스 필터 (`local,jira,confluence,figma`) |
| `--min-score` | 최소 유사도 (0.0-1.0) |
| `-f, --format` | 출력 형식 (`text,json,markdown`) |

---

## 문제 해결

### 상태 확인
```bash
ssearch status
docker ps  # Qdrant 확인
```

### 데몬 재시작
```bash
ssearch serve restart
```

### 디버그
```bash
RUST_LOG=debug ssearch search "query"
```

---

## 지원

- [GitHub Issues](https://github.com/junyeong-ai/semantic-search-cli/issues)
- [개발자 가이드](CLAUDE.md)

---

<div align="center">

**[English](README.en.md)** | **한국어**

Made with Rust

</div>
