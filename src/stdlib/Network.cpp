#include "starry/stdlib/Network.h"
#include <iostream>
#include <sstream>
#include <thread>
#include <chrono>

#ifdef _WIN32
#include <winsock2.h>
#include <ws2tcpip.h>
#pragma comment(lib, "ws2_32.lib")
#else
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <unistd.h>
#endif

namespace starry {
namespace stdlib {

// NetworkManager 类实现
NetworkManager::NetworkManager() : initialized_(false) {}

NetworkManager::~NetworkManager() {
    cleanup();
}

bool NetworkManager::initialize() {
    if (initialized_) return true;
    
#ifdef _WIN32
    WSADATA wsaData;
    int result = WSAStartup(MAKEWORD(2, 2), &wsaData);
    if (result != 0) {
        return false;
    }
#endif
    
    initialized_ = true;
    return true;
}

void NetworkManager::cleanup() {
    if (!initialized_) return;
    
    // 关闭所有连接
    for (auto& pair : connections_) {
        closeConnection(pair.first);
    }
    connections_.clear();
    
#ifdef _WIN32
    WSACleanup();
#endif
    
    initialized_ = false;
}

bool NetworkManager::isInitialized() const {
    return initialized_;
}

// Socket 类实现
Socket::Socket() : socket_(INVALID_SOCKET), connected_(false), listening_(false) {}

Socket::~Socket() {
    close();
}

bool Socket::create(SocketType type) {
    if (!NetworkManager::getInstance().isInitialized()) {
        NetworkManager::getInstance().initialize();
    }
    
    int socketType = (type == SocketType::TCP) ? SOCK_STREAM : SOCK_DGRAM;
    int protocol = (type == SocketType::TCP) ? IPPROTO_TCP : IPPROTO_UDP;
    
#ifdef _WIN32
    socket_ = ::socket(AF_INET, socketType, protocol);
    return socket_ != INVALID_SOCKET;
#else
    socket_ = ::socket(AF_INET, socketType, protocol);
    return socket_ >= 0;
#endif
}

bool Socket::bind(const std::string& address, int port) {
    if (!isValid()) return false;
    
    sockaddr_in addr;
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port);
    
    if (address.empty() || address == "0.0.0.0") {
        addr.sin_addr.s_addr = INADDR_ANY;
    } else {
        inet_pton(AF_INET, address.c_str(), &addr.sin_addr);
    }
    
    int result = ::bind(socket_, reinterpret_cast<sockaddr*>(&addr), sizeof(addr));
    return result == 0;
}

bool Socket::listen(int backlog) {
    if (!isValid()) return false;
    
    int result = ::listen(socket_, backlog);
    if (result == 0) {
        listening_ = true;
    }
    return result == 0;
}

Socket Socket::accept() {
    if (!isValid() || !listening_) {
        return Socket();
    }
    
    sockaddr_in clientAddr;
    socklen_t clientAddrLen = sizeof(clientAddr);
    
#ifdef _WIN32
    SOCKET clientSocket = ::accept(socket_, reinterpret_cast<sockaddr*>(&clientAddr), &clientAddrLen);
    if (clientSocket == INVALID_SOCKET) {
        return Socket();
    }
#else
    int clientSocket = ::accept(socket_, reinterpret_cast<sockaddr*>(&clientAddr), &clientAddrLen);
    if (clientSocket < 0) {
        return Socket();
    }
#endif
    
    Socket client;
    client.socket_ = clientSocket;
    client.connected_ = true;
    
    // 获取客户端地址信息
    char clientIP[INET_ADDRSTRLEN];
    inet_ntop(AF_INET, &clientAddr.sin_addr, clientIP, INET_ADDRSTRLEN);
    client.remoteAddress_ = std::string(clientIP);
    client.remotePort_ = ntohs(clientAddr.sin_port);
    
    return client;
}

bool Socket::connect(const std::string& address, int port) {
    if (!isValid()) return false;
    
    sockaddr_in addr;
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port);
    inet_pton(AF_INET, address.c_str(), &addr.sin_addr);
    
    int result = ::connect(socket_, reinterpret_cast<sockaddr*>(&addr), sizeof(addr));
    if (result == 0) {
        connected_ = true;
        remoteAddress_ = address;
        remotePort_ = port;
    }
    return result == 0;
}

int Socket::send(const std::string& data) {
    if (!isValid() || !connected_) return -1;
    
    int result = ::send(socket_, data.c_str(), static_cast<int>(data.length()), 0);
    return result;
}

int Socket::send(const std::vector<uint8_t>& data) {
    if (!isValid() || !connected_) return -1;
    
    int result = ::send(socket_, reinterpret_cast<const char*>(data.data()), 
                       static_cast<int>(data.size()), 0);
    return result;
}

std::string Socket::receive(int maxLength) {
    if (!isValid() || !connected_) return "";
    
    std::vector<char> buffer(maxLength);
    int received = ::recv(socket_, buffer.data(), maxLength, 0);
    
    if (received > 0) {
        return std::string(buffer.data(), received);
    }
    return "";
}

std::vector<uint8_t> Socket::receiveBytes(int maxLength) {
    if (!isValid() || !connected_) return {};
    
    std::vector<uint8_t> buffer(maxLength);
    int received = ::recv(socket_, reinterpret_cast<char*>(buffer.data()), maxLength, 0);
    
    if (received > 0) {
        buffer.resize(received);
        return buffer;
    }
    return {};
}

int Socket::sendTo(const std::string& data, const std::string& address, int port) {
    if (!isValid()) return -1;
    
    sockaddr_in addr;
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port);
    inet_pton(AF_INET, address.c_str(), &addr.sin_addr);
    
    int result = ::sendto(socket_, data.c_str(), static_cast<int>(data.length()), 0,
                         reinterpret_cast<sockaddr*>(&addr), sizeof(addr));
    return result;
}

std::string Socket::receiveFrom(int maxLength, std::string& fromAddress, int& fromPort) {
    if (!isValid()) return "";
    
    std::vector<char> buffer(maxLength);
    sockaddr_in fromAddr;
    socklen_t fromAddrLen = sizeof(fromAddr);
    
    int received = ::recvfrom(socket_, buffer.data(), maxLength, 0,
                             reinterpret_cast<sockaddr*>(&fromAddr), &fromAddrLen);
    
    if (received > 0) {
        char fromIP[INET_ADDRSTRLEN];
        inet_ntop(AF_INET, &fromAddr.sin_addr, fromIP, INET_ADDRSTRLEN);
        fromAddress = std::string(fromIP);
        fromPort = ntohs(fromAddr.sin_port);
        
        return std::string(buffer.data(), received);
    }
    return "";
}

void Socket::close() {
    if (isValid()) {
#ifdef _WIN32
        closesocket(socket_);
#else
        ::close(socket_);
#endif
        socket_ = INVALID_SOCKET;
        connected_ = false;
        listening_ = false;
    }
}

bool Socket::isValid() const {
#ifdef _WIN32
    return socket_ != INVALID_SOCKET;
#else
    return socket_ >= 0;
#endif
}

bool Socket::isConnected() const {
    return connected_;
}

bool Socket::isListening() const {
    return listening_;
}

std::string Socket::getRemoteAddress() const {
    return remoteAddress_;
}

int Socket::getRemotePort() const {
    return remotePort_;
}

bool Socket::setOption(SocketOption option, int value) {
    if (!isValid()) return false;
    
    int level = SOL_SOCKET;
    int optname;
    
    switch (option) {
        case SocketOption::REUSE_ADDRESS:
            optname = SO_REUSEADDR;
            break;
        case SocketOption::KEEP_ALIVE:
            optname = SO_KEEPALIVE;
            break;
        case SocketOption::RECEIVE_TIMEOUT:
#ifdef _WIN32
            optname = SO_RCVTIMEO;
#else
            optname = SO_RCVTIMEO;
#endif
            break;
        case SocketOption::SEND_TIMEOUT:
#ifdef _WIN32
            optname = SO_SNDTIMEO;
#else
            optname = SO_SNDTIMEO;
#endif
            break;
        default:
            return false;
    }
    
    int result = setsockopt(socket_, level, optname, 
                           reinterpret_cast<const char*>(&value), sizeof(value));
    return result == 0;
}

int Socket::getOption(SocketOption option) {
    if (!isValid()) return -1;
    
    int level = SOL_SOCKET;
    int optname;
    
    switch (option) {
        case SocketOption::REUSE_ADDRESS:
            optname = SO_REUSEADDR;
            break;
        case SocketOption::KEEP_ALIVE:
            optname = SO_KEEPALIVE;
            break;
        case SocketOption::RECEIVE_TIMEOUT:
            optname = SO_RCVTIMEO;
            break;
        case SocketOption::SEND_TIMEOUT:
            optname = SO_SNDTIMEO;
            break;
        default:
            return -1;
    }
    
    int value;
    socklen_t valueLen = sizeof(value);
    int result = getsockopt(socket_, level, optname, 
                           reinterpret_cast<char*>(&value), &valueLen);
    
    return (result == 0) ? value : -1;
}

// HttpClient 类实现
HttpClient::HttpClient() : timeout_(30000) {}

HttpResponse HttpClient::get(const std::string& url) {
    return request("GET", url, {}, "");
}

HttpResponse HttpClient::post(const std::string& url, const std::string& data) {
    return request("POST", url, {}, data);
}

HttpResponse HttpClient::put(const std::string& url, const std::string& data) {
    return request("PUT", url, {}, data);
}

HttpResponse HttpClient::del(const std::string& url) {
    return request("DELETE", url, {}, "");
}

HttpResponse HttpClient::request(const std::string& method, const std::string& url,
                                const std::map<std::string, std::string>& headers,
                                const std::string& body) {
    HttpResponse response;
    
    // 解析URL
    std::string host, path;
    int port = 80;
    bool isHttps = false;
    
    if (!parseUrl(url, host, port, path, isHttps)) {
        response.statusCode = -1;
        response.statusMessage = "Invalid URL";
        return response;
    }
    
    // 创建socket连接
    Socket socket;
    if (!socket.create(SocketType::TCP)) {
        response.statusCode = -1;
        response.statusMessage = "Failed to create socket";
        return response;
    }
    
    if (!socket.connect(host, port)) {
        response.statusCode = -1;
        response.statusMessage = "Failed to connect to server";
        return response;
    }
    
    // 构建HTTP请求
    std::ostringstream requestStream;
    requestStream << method << " " << path << " HTTP/1.1\r\n";
    requestStream << "Host: " << host << "\r\n";
    requestStream << "Connection: close\r\n";
    
    // 添加自定义头部
    for (const auto& header : headers) {
        requestStream << header.first << ": " << header.second << "\r\n";
    }
    
    // 添加Content-Length（如果有body）
    if (!body.empty()) {
        requestStream << "Content-Length: " << body.length() << "\r\n";
    }
    
    requestStream << "\r\n";
    
    // 添加body
    if (!body.empty()) {
        requestStream << body;
    }
    
    std::string request = requestStream.str();
    
    // 发送请求
    if (socket.send(request) <= 0) {
        response.statusCode = -1;
        response.statusMessage = "Failed to send request";
        return response;
    }
    
    // 接收响应
    std::string responseData;
    std::string chunk;
    while (!(chunk = socket.receive(4096)).empty()) {
        responseData += chunk;
    }
    
    // 解析响应
    parseResponse(responseData, response);
    
    return response;
}

void HttpClient::setTimeout(int timeoutMs) {
    timeout_ = timeoutMs;
}

int HttpClient::getTimeout() const {
    return timeout_;
}

bool HttpClient::parseUrl(const std::string& url, std::string& host, int& port, 
                         std::string& path, bool& isHttps) {
    // 简化的URL解析实现
    std::string urlCopy = url;
    
    // 检查协议
    if (urlCopy.substr(0, 8) == "https://") {
        isHttps = true;
        port = 443;
        urlCopy = urlCopy.substr(8);
    } else if (urlCopy.substr(0, 7) == "http://") {
        isHttps = false;
        port = 80;
        urlCopy = urlCopy.substr(7);
    } else {
        return false;
    }
    
    // 查找路径分隔符
    size_t pathPos = urlCopy.find('/');
    if (pathPos == std::string::npos) {
        host = urlCopy;
        path = "/";
    } else {
        host = urlCopy.substr(0, pathPos);
        path = urlCopy.substr(pathPos);
    }
    
    // 检查端口号
    size_t portPos = host.find(':');
    if (portPos != std::string::npos) {
        std::string portStr = host.substr(portPos + 1);
        host = host.substr(0, portPos);
        port = std::stoi(portStr);
    }
    
    return true;
}

void HttpClient::parseResponse(const std::string& responseData, HttpResponse& response) {
    if (responseData.empty()) {
        response.statusCode = -1;
        response.statusMessage = "Empty response";
        return;
    }
    
    // 查找头部和body的分隔符
    size_t headerEndPos = responseData.find("\r\n\r\n");
    if (headerEndPos == std::string::npos) {
        response.statusCode = -1;
        response.statusMessage = "Invalid response format";
        return;
    }
    
    std::string headerSection = responseData.substr(0, headerEndPos);
    response.body = responseData.substr(headerEndPos + 4);
    
    // 解析状态行
    std::istringstream headerStream(headerSection);
    std::string statusLine;
    std::getline(headerStream, statusLine);
    
    std::istringstream statusStream(statusLine);
    std::string httpVersion;
    statusStream >> httpVersion >> response.statusCode >> response.statusMessage;
    
    // 解析头部
    std::string headerLine;
    while (std::getline(headerStream, headerLine) && !headerLine.empty()) {
        // 移除回车符
        if (!headerLine.empty() && headerLine.back() == '\r') {
            headerLine.pop_back();
        }
        
        size_t colonPos = headerLine.find(':');
        if (colonPos != std::string::npos) {
            std::string key = headerLine.substr(0, colonPos);
            std::string value = headerLine.substr(colonPos + 1);
            
            // 去除前后空格
            while (!key.empty() && key.back() == ' ') key.pop_back();
            while (!value.empty() && value.front() == ' ') value.erase(0, 1);
            while (!value.empty() && value.back() == ' ') value.pop_back();
            
            response.headers[key] = value;
        }
    }
}

// NetworkManager 单例实现
NetworkManager& NetworkManager::getInstance() {
    static NetworkManager instance;
    return instance;
}

// 全局便利函数
bool initializeNetwork() {
    return NetworkManager::getInstance().initialize();
}

void cleanupNetwork() {
    NetworkManager::getInstance().cleanup();
}

Socket createTcpSocket() {
    Socket socket;
    socket.create(SocketType::TCP);
    return socket;
}

Socket createUdpSocket() {
    Socket socket;
    socket.create(SocketType::UDP);
    return socket;
}

HttpResponse httpGet(const std::string& url) {
    HttpClient client;
    return client.get(url);
}

HttpResponse httpPost(const std::string& url, const std::string& data) {
    HttpClient client;
    return client.post(url, data);
}

std::string resolveHostname(const std::string& hostname) {
    struct hostent* host = gethostbyname(hostname.c_str());
    if (host && host->h_addr_list[0]) {
        struct in_addr addr;
        memcpy(&addr, host->h_addr_list[0], sizeof(struct in_addr));
        return std::string(inet_ntoa(addr));
    }
    return "";
}

bool isValidIpAddress(const std::string& ip) {
    struct sockaddr_in sa;
    int result = inet_pton(AF_INET, ip.c_str(), &(sa.sin_addr));
    return result != 0;
}

} // namespace stdlib
} // namespace starry