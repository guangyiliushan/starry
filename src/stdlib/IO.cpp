#include "starry/stdlib/IO.h"
#include <iostream>
#include <fstream>
#include <sstream>
#include <filesystem>
#include <algorithm>

namespace starry {
namespace stdlib {

// Console 类实现
Console::Console() {}

void Console::print(const std::string& message) {
    std::cout << message;
    std::cout.flush();
}

void Console::println(const std::string& message) {
    std::cout << message << std::endl;
}

void Console::printError(const std::string& message) {
    std::cerr << message;
    std::cerr.flush();
}

void Console::printErrorLine(const std::string& message) {
    std::cerr << message << std::endl;
}

std::string Console::readLine() {
    std::string line;
    std::getline(std::cin, line);
    return line;
}

std::string Console::readLine(const std::string& prompt) {
    print(prompt);
    return readLine();
}

char Console::readChar() {
    char ch;
    std::cin >> ch;
    return ch;
}

int Console::readInt() {
    int value;
    std::cin >> value;
    return value;
}

int Console::readInt(const std::string& prompt) {
    print(prompt);
    return readInt();
}

double Console::readDouble() {
    double value;
    std::cin >> value;
    return value;
}

double Console::readDouble(const std::string& prompt) {
    print(prompt);
    return readDouble();
}

bool Console::readBool() {
    std::string input;
    std::cin >> input;
    std::transform(input.begin(), input.end(), input.begin(), ::tolower);
    return input == "true" || input == "1" || input == "yes" || input == "y";
}

bool Console::readBool(const std::string& prompt) {
    print(prompt);
    return readBool();
}

void Console::clear() {
#ifdef _WIN32
    system("cls");
#else
    system("clear");
#endif
}

void Console::setColor(ConsoleColor color) {
#ifdef _WIN32
    HANDLE hConsole = GetStdHandle(STD_OUTPUT_HANDLE);
    SetConsoleTextAttribute(hConsole, static_cast<WORD>(color));
#else
    // ANSI颜色代码
    switch (color) {
        case ConsoleColor::BLACK:
            std::cout << "\033[30m";
            break;
        case ConsoleColor::RED:
            std::cout << "\033[31m";
            break;
        case ConsoleColor::GREEN:
            std::cout << "\033[32m";
            break;
        case ConsoleColor::YELLOW:
            std::cout << "\033[33m";
            break;
        case ConsoleColor::BLUE:
            std::cout << "\033[34m";
            break;
        case ConsoleColor::MAGENTA:
            std::cout << "\033[35m";
            break;
        case ConsoleColor::CYAN:
            std::cout << "\033[36m";
            break;
        case ConsoleColor::WHITE:
            std::cout << "\033[37m";
            break;
        default:
            std::cout << "\033[0m"; // 重置
            break;
    }
#endif
}

void Console::resetColor() {
#ifdef _WIN32
    HANDLE hConsole = GetStdHandle(STD_OUTPUT_HANDLE);
    SetConsoleTextAttribute(hConsole, 7); // 默认白色
#else
    std::cout << "\033[0m";
#endif
}

// File 类实现
File::File() : isOpen_(false) {}

File::File(const std::string& filename) : filename_(filename), isOpen_(false) {}

File::~File() {
    close();
}

bool File::open(const std::string& filename, FileMode mode) {
    filename_ = filename;
    return open(mode);
}

bool File::open(FileMode mode) {
    if (isOpen_) {
        close();
    }
    
    std::ios::openmode openMode = std::ios::in;
    switch (mode) {
        case FileMode::READ:
            openMode = std::ios::in;
            break;
        case FileMode::WRITE:
            openMode = std::ios::out;
            break;
        case FileMode::APPEND:
            openMode = std::ios::out | std::ios::app;
            break;
        case FileMode::READ_WRITE:
            openMode = std::ios::in | std::ios::out;
            break;
        case FileMode::BINARY_READ:
            openMode = std::ios::in | std::ios::binary;
            break;
        case FileMode::BINARY_WRITE:
            openMode = std::ios::out | std::ios::binary;
            break;
        case FileMode::BINARY_READ_WRITE:
            openMode = std::ios::in | std::ios::out | std::ios::binary;
            break;
    }
    
    fileStream_.open(filename_, openMode);
    isOpen_ = fileStream_.is_open();
    return isOpen_;
}

void File::close() {
    if (isOpen_) {
        fileStream_.close();
        isOpen_ = false;
    }
}

bool File::isOpen() const {
    return isOpen_;
}

std::string File::readAll() {
    if (!isOpen_) return "";
    
    std::ostringstream buffer;
    buffer << fileStream_.rdbuf();
    return buffer.str();
}

std::string File::readLine() {
    if (!isOpen_) return "";
    
    std::string line;
    std::getline(fileStream_, line);
    return line;
}

std::vector<std::string> File::readLines() {
    std::vector<std::string> lines;
    if (!isOpen_) return lines;
    
    std::string line;
    while (std::getline(fileStream_, line)) {
        lines.push_back(line);
    }
    return lines;
}

std::vector<char> File::readBytes(size_t count) {
    std::vector<char> buffer(count);
    if (!isOpen_) return buffer;
    
    fileStream_.read(buffer.data(), count);
    size_t actualRead = fileStream_.gcount();
    buffer.resize(actualRead);
    return buffer;
}

bool File::write(const std::string& content) {
    if (!isOpen_) return false;
    
    fileStream_ << content;
    return fileStream_.good();
}

bool File::writeLine(const std::string& line) {
    if (!isOpen_) return false;
    
    fileStream_ << line << std::endl;
    return fileStream_.good();
}

bool File::writeLines(const std::vector<std::string>& lines) {
    if (!isOpen_) return false;
    
    for (const auto& line : lines) {
        fileStream_ << line << std::endl;
        if (!fileStream_.good()) return false;
    }
    return true;
}

bool File::writeBytes(const std::vector<char>& data) {
    if (!isOpen_) return false;
    
    fileStream_.write(data.data(), data.size());
    return fileStream_.good();
}

void File::flush() {
    if (isOpen_) {
        fileStream_.flush();
    }
}

size_t File::size() const {
    if (!exists(filename_)) return 0;
    
    try {
        return std::filesystem::file_size(filename_);
    } catch (const std::filesystem::filesystem_error&) {
        return 0;
    }
}

bool File::eof() const {
    return !isOpen_ || fileStream_.eof();
}

void File::seek(size_t position) {
    if (isOpen_) {
        fileStream_.seekg(position);
        fileStream_.seekp(position);
    }
}

size_t File::tell() const {
    if (!isOpen_) return 0;
    return fileStream_.tellg();
}

// 静态方法实现
bool File::exists(const std::string& filename) {
    return std::filesystem::exists(filename);
}

bool File::remove(const std::string& filename) {
    try {
        return std::filesystem::remove(filename);
    } catch (const std::filesystem::filesystem_error&) {
        return false;
    }
}

bool File::copy(const std::string& source, const std::string& destination) {
    try {
        std::filesystem::copy_file(source, destination, 
                                  std::filesystem::copy_options::overwrite_existing);
        return true;
    } catch (const std::filesystem::filesystem_error&) {
        return false;
    }
}

bool File::move(const std::string& source, const std::string& destination) {
    try {
        std::filesystem::rename(source, destination);
        return true;
    } catch (const std::filesystem::filesystem_error&) {
        return false;
    }
}

std::string File::getExtension(const std::string& filename) {
    std::filesystem::path path(filename);
    return path.extension().string();
}

std::string File::getName(const std::string& filename) {
    std::filesystem::path path(filename);
    return path.filename().string();
}

std::string File::getDirectory(const std::string& filename) {
    std::filesystem::path path(filename);
    return path.parent_path().string();
}

// Directory 类实现
Directory::Directory() {}

Directory::Directory(const std::string& path) : path_(path) {}

bool Directory::exists() const {
    return exists(path_);
}

bool Directory::create() {
    return create(path_);
}

bool Directory::remove() {
    return remove(path_);
}

std::vector<std::string> Directory::listFiles() const {
    return listFiles(path_);
}

std::vector<std::string> Directory::listDirectories() const {
    return listDirectories(path_);
}

std::vector<std::string> Directory::listAll() const {
    return listAll(path_);
}

void Directory::setPath(const std::string& path) {
    path_ = path;
}

std::string Directory::getPath() const {
    return path_;
}

// 静态方法实现
bool Directory::exists(const std::string& path) {
    return std::filesystem::exists(path) && std::filesystem::is_directory(path);
}

bool Directory::create(const std::string& path) {
    try {
        return std::filesystem::create_directories(path);
    } catch (const std::filesystem::filesystem_error&) {
        return false;
    }
}

bool Directory::remove(const std::string& path) {
    try {
        return std::filesystem::remove_all(path) > 0;
    } catch (const std::filesystem::filesystem_error&) {
        return false;
    }
}

std::vector<std::string> Directory::listFiles(const std::string& path) {
    std::vector<std::string> files;
    
    try {
        for (const auto& entry : std::filesystem::directory_iterator(path)) {
            if (entry.is_regular_file()) {
                files.push_back(entry.path().string());
            }
        }
    } catch (const std::filesystem::filesystem_error&) {
        // 忽略错误，返回空列表
    }
    
    return files;
}

std::vector<std::string> Directory::listDirectories(const std::string& path) {
    std::vector<std::string> directories;
    
    try {
        for (const auto& entry : std::filesystem::directory_iterator(path)) {
            if (entry.is_directory()) {
                directories.push_back(entry.path().string());
            }
        }
    } catch (const std::filesystem::filesystem_error&) {
        // 忽略错误，返回空列表
    }
    
    return directories;
}

std::vector<std::string> Directory::listAll(const std::string& path) {
    std::vector<std::string> entries;
    
    try {
        for (const auto& entry : std::filesystem::directory_iterator(path)) {
            entries.push_back(entry.path().string());
        }
    } catch (const std::filesystem::filesystem_error&) {
        // 忽略错误，返回空列表
    }
    
    return entries;
}

std::string Directory::getCurrentDirectory() {
    try {
        return std::filesystem::current_path().string();
    } catch (const std::filesystem::filesystem_error&) {
        return "";
    }
}

bool Directory::setCurrentDirectory(const std::string& path) {
    try {
        std::filesystem::current_path(path);
        return true;
    } catch (const std::filesystem::filesystem_error&) {
        return false;
    }
}

// Path 类实现
Path::Path() {}

Path::Path(const std::string& path) : path_(path) {}

std::string Path::toString() const {
    return path_;
}

std::string Path::getFileName() const {
    std::filesystem::path path(path_);
    return path.filename().string();
}

std::string Path::getExtension() const {
    std::filesystem::path path(path_);
    return path.extension().string();
}

std::string Path::getDirectory() const {
    std::filesystem::path path(path_);
    return path.parent_path().string();
}

std::string Path::getAbsolutePath() const {
    try {
        return std::filesystem::absolute(path_).string();
    } catch (const std::filesystem::filesystem_error&) {
        return path_;
    }
}

bool Path::isAbsolute() const {
    std::filesystem::path path(path_);
    return path.is_absolute();
}

bool Path::isRelative() const {
    std::filesystem::path path(path_);
    return path.is_relative();
}

Path Path::join(const std::string& other) const {
    std::filesystem::path path(path_);
    path /= other;
    return Path(path.string());
}

Path Path::normalize() const {
    try {
        std::filesystem::path path(path_);
        return Path(std::filesystem::weakly_canonical(path).string());
    } catch (const std::filesystem::filesystem_error&) {
        return *this;
    }
}

// 静态方法实现
Path Path::combine(const std::string& path1, const std::string& path2) {
    std::filesystem::path path(path1);
    path /= path2;
    return Path(path.string());
}

std::string Path::getDirectorySeparator() {
    return std::string(1, std::filesystem::path::preferred_separator);
}

// 全局便利函数
void print(const std::string& message) {
    Console console;
    console.print(message);
}

void println(const std::string& message) {
    Console console;
    console.println(message);
}

std::string readLine() {
    Console console;
    return console.readLine();
}

std::string readLine(const std::string& prompt) {
    Console console;
    return console.readLine(prompt);
}

int readInt() {
    Console console;
    return console.readInt();
}

int readInt(const std::string& prompt) {
    Console console;
    return console.readInt(prompt);
}

double readDouble() {
    Console console;
    return console.readDouble();
}

double readDouble(const std::string& prompt) {
    Console console;
    return console.readDouble(prompt);
}

std::string readFile(const std::string& filename) {
    File file(filename);
    if (file.open(FileMode::READ)) {
        return file.readAll();
    }
    return "";
}

bool writeFile(const std::string& filename, const std::string& content) {
    File file(filename);
    if (file.open(FileMode::WRITE)) {
        return file.write(content);
    }
    return false;
}

std::vector<std::string> readLines(const std::string& filename) {
    File file(filename);
    if (file.open(FileMode::READ)) {
        return file.readLines();
    }
    return {};
}

bool writeLines(const std::string& filename, const std::vector<std::string>& lines) {
    File file(filename);
    if (file.open(FileMode::WRITE)) {
        return file.writeLines(lines);
    }
    return false;
}

} // namespace stdlib
} // namespace starry