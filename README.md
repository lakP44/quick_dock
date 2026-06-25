# Quick Dock

Rust + egui/eframe로 만든 작은 도킹 런처입니다. 화면 가장자리에 붙여 두고 복사, 실행, 파일/폴더 열기 작업을 빠르게 누르는 용도입니다.

## 기능

- 화면 왼쪽, 오른쪽, 위, 아래 가장자리에 도킹
- 마우스를 올리면 확장, 벗어나면 접힘
- 탭별 작업 관리
- 텍스트 클립보드 복사
- 프로그램 실행
- 파일/폴더 열기
- 앱 안에서 설정 편집
- 실행/열기 경로 검증, 테스트 실행, 복사 테스트
- 설정 저장 전 `env\quick_dock.ini.bak` 자동 백업

## 빌드

```bat
cargo build --release
```

빌드 결과:

```text
target\release\quick_dock.exe
```

배포용 빌드는 다음 스크립트를 사용할 수 있습니다.

```bat
build_windows_release.bat
```

이 스크립트는 `env` 폴더의 파일을 `target\release\env`로 복사하되, 이미 존재하는 사용자 설정 파일은 덮어쓰지 않습니다.

## 설정 파일

Quick Dock은 실행 파일 옆의 `env\quick_dock.ini`를 사용합니다. 앱의 설정 화면에서 편집하는 것을 권장합니다.

```ini
[layout]
schema_version=1
expanded_width=350
expanded_height=430

[tab.1]
name=기본

[tab.1.item.1]
kind=copy_text
name=Jira - 검토 완료
text=검토 완료했습니다.\n\n확인 내용:\n- \n

[tab.1.item.2]
kind=run_app
name=메모장 실행
command=notepad.exe
arguments=

[tab.1.item.3]
kind=open_path
name=다운로드 폴더 열기
path=C:\Users\%USERNAME%\Downloads
```

`arguments`는 `값1|값2|값3`처럼 `|`로 구분합니다. 줄바꿈은 `\n`으로 저장됩니다.

## 조작

- 상단 `Quick Dock` 제목줄 드래그: 창 이동 또는 가장자리 도킹
- `새 탭`: 탭 추가
- `설정`: 현재 탭 작업 편집
- 설정 저장: 기존 `quick_dock.ini`를 `quick_dock.ini.bak`로 백업한 뒤 저장
- 실행 항목 `찾아보기...`: exe 파일 선택
- 실행/열기 항목 `테스트`: 저장 전 실제 동작 확인
