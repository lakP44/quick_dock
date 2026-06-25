# Quick Dock

Rust + egui/eframe으로 만든 작은 도킹 패널 예제입니다.

## 기능

- 항상 위에 표시되는 작은 패널
- 화면 왼쪽/오른쪽/위/아래 가장자리에 붙는 구조
- 마우스를 올리면 확장, 벗어나면 축소
- 헤더를 드래그하면 위치 이동
- 드래그가 끝나면 가장 가까운 화면 가장자리에 스냅
- TOML 설정 파일에서 버튼 관리
- 텍스트 클립보드 복사
- 프로그램 실행
- 폴더/파일 열기

## 빌드

```bat
cargo build --release
```

빌드 결과:

```text
target\release\quick_dock.exe
```

`quick_dock.toml` 파일을 `quick_dock.exe`와 같은 폴더에 복사해서 사용하세요.

## 설정 예시

```toml
[[items]]
kind = "copy_text"
name = "Jira - 검토 완료"
text = """
검토 완료했습니다.

확인 내용:
- 

조치 사항:
- 없음
"""

[[items]]
kind = "run_app"
name = "메모장 실행"
command = "notepad.exe"
arguments = []

[[items]]
kind = "open_path"
name = "다운로드 폴더 열기"
path = "C:\\Users\\%USERNAME%\\Downloads"
```

## 조작

- 패널 위에 마우스 올림: 확장
- 패널 밖으로 마우스 이동: 축소
- 상단 `Quick Dock` 헤더 드래그: 이동
- `다시 읽기`: `quick_dock.toml` 재로드
- `종료`: 앱 종료
