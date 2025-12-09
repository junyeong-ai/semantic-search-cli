# Semantic Search CLI

[![Rust](https://img.shields.io/badge/rust-1.91.1%2B%20(2024%20edition)-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![CI](https://img.shields.io/github/actions/workflow/status/junyeong-ai/semantic-search-cli/ci.yml?branch=main&style=flat-square&logo=github&label=CI)](https://github.com/junyeong-ai/semantic-search-cli/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/junyeong-ai/semantic-search-cli?style=flat-square&logo=github)](https://github.com/junyeong-ai/semantic-search-cli/releases/latest)

> **🌐 한국어** | **[English](README.en.md)**

---

> **🔍 AI 기반 시맨틱 검색 CLI**
>
> - 🧠 **의미 기반 검색** (Qwen3 임베딩 1024차원 + Qdrant 벡터 DB)
> - 📁 **로컬 파일 인덱싱** (코드, 문서, 설정 파일)
> - 🔗 **외부 소스 통합** (Jira, Confluence, Figma)
> - 🏷️ **태그 필터링** (소스별, 언어별, 프로젝트별)

---

## ⚡ 빠른 시작 (5분)

```bash
# 1. 설치
git clone https://github.com/junyeong-ai/semantic-search-cli
cd semantic-search-cli
./scripts/install.sh

# 2. 인프라 시작
docker-compose up -d qdrant
cd embedding-server && python server.py &

# 3. 상태 확인
ssearch status

# 4. 파일 인덱싱
ssearch index add ./src

# 5. 검색! 🎉
ssearch search "사용자 인증 로직"
```

---

## 🎯 주요 기능

### 시맨틱 검색
```bash
# 기본 검색
ssearch search "API 엔드포인트 설계"

# 태그 필터링
ssearch search "결제 처리" --tags "source:jira"

# 소스 타입 필터링
ssearch search "에러 핸들링" --source jira,confluence

# JSON 출력
ssearch search "에러 핸들링" --format json --limit 5

# 최소 점수 필터링
ssearch search "인증 로직" --min-score 0.7
```

### 파일 인덱싱
```bash
# 디렉토리 인덱싱
ssearch index add ./src --tags "project:myapp"

# 특정 패턴 제외
ssearch index add . -e "node_modules" -e "target" -e ".git"
```

### 외부 소스 동기화
```bash
# Jira 프로젝트 전체 동기화 (스트리밍)
ssearch source sync jira --project MYPROJ --all
ssearch source sync jira --project MYPROJ --limit 100

# Jira 이슈 (JQL 또는 이슈 키)
ssearch source sync jira --query "status=Done" --limit 50
ssearch source sync jira --query "PROJ-1234"

# Confluence 스페이스 전체 동기화 (스트리밍)
ssearch source sync confluence --project DOCS --all
ssearch source sync confluence --project DOCS --limit 100

# Confluence 페이지 (CQL 또는 페이지 ID/URL)
ssearch source sync confluence --query "text~keyword" --limit 50

# Figma 디자인 (URL)
ssearch source sync figma --query "https://figma.com/design/xxx?node-id=123"
```

### 관리
```bash
ssearch status            # 인프라 상태 확인
ssearch tags list         # 태그 목록
ssearch index clear -y    # 전체 데이터 삭제
```

---

## 📦 설치

### 전제 조건

| 구성 요소 | 용도 |
|----------|------|
| **Docker** | Qdrant 벡터 DB |
| **Python 3.10+** | 임베딩 서버 |

### 방법 1: 릴리즈 다운로드 (권장)

```bash
# macOS (Apple Silicon)
curl -L https://github.com/junyeong-ai/semantic-search-cli/releases/latest/download/ssearch-$(curl -s https://api.github.com/repos/user/semantic-search-cli/releases/latest | grep tag_name | cut -d '"' -f 4)-aarch64-apple-darwin.tar.gz | tar xz
sudo mv ssearch /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/junyeong-ai/semantic-search-cli/releases/latest/download/ssearch-$(curl -s https://api.github.com/repos/user/semantic-search-cli/releases/latest | grep tag_name | cut -d '"' -f 4)-x86_64-apple-darwin.tar.gz | tar xz
sudo mv ssearch /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/junyeong-ai/semantic-search-cli/releases/latest/download/ssearch-$(curl -s https://api.github.com/repos/user/semantic-search-cli/releases/latest | grep tag_name | cut -d '"' -f 4)-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv ssearch /usr/local/bin/
```

### 방법 2: 소스에서 빌드

```bash
git clone https://github.com/junyeong-ai/semantic-search-cli
cd semantic-search-cli
./scripts/install.sh
```

> **요구사항**: Rust 1.91.1+

### 방법 3: 수동 빌드

```bash
cargo build --release
cp target/release/ssearch ~/.local/bin/
```

### 🤖 Claude Code 스킬

설치 시 Claude Code 스킬 설치 여부 선택 가능:
- **User-level**: 모든 프로젝트에서 사용 가능
- **Project-level**: Git을 통해 팀 자동 배포

---

## ⚙️ 설정

### 인프라 시작

```bash
# Qdrant (벡터 DB)
docker-compose up -d qdrant

# 임베딩 서버 (Qwen3)
cd embedding-server && python server.py
```

### 설정 파일

**위치**: `~/.config/semantic-search-cli/config.toml`

```toml
[embedding]
url = "http://localhost:11411"
timeout_secs = 120
batch_size = 8

[vector_store]
url = "http://localhost:16334"
collection = "semantic_search"
# api_key = "optional-api-key"

[indexing]
max_file_size = 10485760  # 10MB
chunk_size = 6000
chunk_overlap = 500
exclude_patterns = [
  "**/node_modules/**",
  "**/target/**",
  "**/.git/**",
]

[search]
default_limit = 10
default_format = "text"  # text, json, markdown
# default_min_score = 0.5
```

---

## 📚 명령어 참조

| 명령어 | 설명 |
|--------|------|
| `search <query>` | 시맨틱 검색 |
| `index add <path>` | 파일 인덱싱 |
| `index delete <path>` | 인덱스에서 삭제 |
| `index clear` | 전체 인덱스 삭제 |
| `source sync <type>` | 외부 소스 동기화 |
| `source list` | 소스 목록 |
| `source delete <type>` | 소스별 데이터 삭제 |
| `status` | 인프라 상태 확인 |
| `tags list` | 태그 목록 |
| `tags delete <tag>` | 태그별 데이터 삭제 |
| `import <file>` | JSON/JSONL 가져오기 |
| `config init` | 설정 초기화 |
| `config show` | 현재 설정 표시 |
| `config edit` | 설정 파일 편집 |

### 검색 옵션

| 옵션 | 설명 |
|------|------|
| `-n, --limit N` | 결과 수 제한 (기본: 10) |
| `-t, --tags "k:v"` | 태그 필터링 |
| `-s, --source type` | 소스 타입 필터링 (local, jira, confluence, figma) |
| `--min-score` | 최소 유사도 점수 |
| `--format` | 출력 형식 (text/json/markdown) |

---

## 🔧 문제 해결

### 연결 오류
```bash
ssearch status  # 인프라 상태 확인
docker ps       # Qdrant 실행 확인
```

### 검색 결과 없음
- 인덱싱 여부 확인: `ssearch status`
- 인프라 실행 확인: Qdrant + 임베딩 서버

### 인덱싱 실패
- 임베딩 서버 확인: `curl localhost:11411/health`

---

## 💬 지원

- **GitHub Issues**: 문제 신고
- **개발자 문서**: [CLAUDE.md](CLAUDE.md)

---

<div align="center">

**🌐 한국어** | **[English](README.en.md)**

**Version 0.1.0** • Rust 2024 Edition

</div>
