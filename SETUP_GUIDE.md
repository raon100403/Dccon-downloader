# 설치 가이드

## 현재 상태

Tauri 프로젝트가 성공적으로 생성되었습니다. 다음 단계를 진행하기 전에 시스템 의존성을 설치해야 합니다.

## 시스템 의존성 설치 (필수!)

현재 시스템: **Fedora**

다음 명령어를 실행하여 필요한 라이브러리를 설치하세요:

```bash
sudo dnf install -y webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel
```

설치 후 다음 명령어로 확인:

```bash
pkg-config --modversion webkit2gtk-4.1
```

## 개발 진행 상황

### ✅ 완료된 작업
1. Tauri + React + TypeScript 프로젝트 생성
2. 기본 구조 설정
3. 아이콘 생성
4. 설정 파일 작성

### 🔄 다음 단계
1. 시스템 의존성 설치 (위 참조)
2. URL 파싱 Rust 커맨드 구현
3. 디시콘 API 통신 구현
4. UI 컴포넌트 작성
5. 다운로드 기능 구현

## 테스트 방법

의존성 설치 후 다음 명령어로 앱을 실행할 수 있습니다:

```bash
cd dccon-downloader
npm run tauri:dev
```

## 문제 해결

### 1. webkit2gtk를 찾을 수 없음
```
The system library `webkit2gtk-4.1` required by crate was not found
```
**해결:** 위의 시스템 의존성 설치 명령어를 실행하세요.

### 2. pkg-config 에러
```
Package 'cairo' not found
```
**해결:** gtk3 및 개발 라이브러리가 설치되지 않았습니다. 위의 명령어를 실행하세요.

### 3. Permission denied
```
sudo: 암호를 읽으려면 터미널이 필요합니다
```
**해결:** 터미널에서 직접 sudo 명령어를 실행해야 합니다.
