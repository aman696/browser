# Custom Web Browser (C++ Project)

## 📌 Project Overview
This is a **foundational web browser** built from scratch using **C++** without leveraging existing browser engines like Chromium, Gecko, or WebKit. The project focuses on developing a modular browser with components for **networking (HTTP/HTTPS requests), HTML parsing, rendering, JavaScript execution, and multimedia playback**.

This repository is a **long-term learning project**, following a structured **phase-based development approach**.

---

## **🚀 Phase 1: Initial Setup & Build System Configuration**

### **1️⃣ Prerequisites**
Before setting up the project, ensure you have the following tools installed:

### **🔹 Windows Users (MinGW + Git Bash)**
- [Git](https://git-scm.com/downloads)
- [MinGW-w64](https://www.mingw-w64.org/downloads/) (for GCC and Makefiles)
- [CMake](https://cmake.org/download/)
- [MSYS2 (if needed)](https://www.msys2.org/)

#### **Verify Installation**
Run the following commands in **Git Bash** or **Command Prompt**:
```bash
# Check if Git is installed
git --version

# Check if GCC is installed
g++ --version

# Check if CMake is installed
cmake --version

# Check if MinGW Makefiles is installed
mingw32-make --version
```
If any of these return "command not found," install the missing tool before proceeding.

### **🔹 macOS/Linux Users**
- [Git](https://git-scm.com/downloads)
- GCC (pre-installed on most Linux distros, or install via Homebrew `brew install gcc`)
- [CMake](https://cmake.org/download/)

Verify installations with:
```bash
git --version
g++ --version
cmake --version
make --version
```

## **3️⃣ Project Structure**
After cloning, the project structure will look like this:
```
browser/
├── src/                  # Source code
│   ├── main.cpp          # Entry point
│   ├── Network/          # Networking Module (HTTP/HTTPS)
│   ├── Parser/           # HTML Parser and DOM Engine
│   ├── Renderer/         # Rendering Engine
│   ├── JS_Engine/        # JavaScript Engine
│   ├── Multimedia/       # Video and Audio Player
│   └── Security/         # Privacy and Security Module
├── include/              # Header files
├── tests/                # Unit and Integration Tests
├── CMakeLists.txt        # CMake Build Configuration
├── .gitignore            # Ignored files
├── README.md             # Project documentation
```

---

## **4️⃣ Building the Project**

### **🔹 Windows (Git Bash or Command Prompt)**
```bash
# Create build directory
mkdir build && cd build

# Run CMake to generate MinGW Makefiles
cmake -G "MinGW Makefiles" ..

# Compile the project
mingw32-make
```

### **🔹 macOS/Linux (Terminal)**
```bash
mkdir build && cd build
cmake ..
make
```

### **🟢 Run the Executable**
After a successful build, run:
```bash
# Windows (Git Bash or CMD)
./browser.exe

# macOS/Linux
./browser
```
Expected Output:
```
Custom Web Browser Starting...
```

---

## **5️⃣ Future Phases (Planned)**

### **✅ Phase 2: Architectural Design & Core Modules** *(Next)*
- Define class structures for:
  - **Networking (HttpClient)** – Handles fetching web pages.
  - **HTML Parser (HtmlParser)** – Converts raw HTML into a DOM tree.
  - **Rendering Engine (Renderer)** – Converts DOM to pixels.
  - **JavaScript Engine** – Executes JavaScript for dynamic pages.
  - **Security Module** – Manages cookies, HTTPS, and XSS protection.

### **🔜 Future Phases**
- **Phase 3:** Implement HTTP Fetching (Basic Browser Navigation)
- **Phase 4:** HTML Parsing & DOM Tree Construction
- **Phase 5:** Rendering Engine (CSS Layouts)
- **Phase 6:** JavaScript Execution
- **Phase 7:** Multimedia Support (Images & Videos)
- **Phase 8:** Privacy & Security Features

---

## **💡 Contributing**
Feel free to fork this project, submit pull requests, or suggest improvements via issues.

---

## **📞 Need Help?**
If you run into issues, open a GitHub issue or reach out to caman1744@gmail.com!

Happy coding! 🚀

