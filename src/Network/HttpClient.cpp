#include "Network/HttpClient.h"
#include "Network/URLParser.h"

// Windows Networking
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <winsock2.h>
#include <ws2tcpip.h>  // For getaddrinfo, etc.
#pragma comment(lib, "ws2_32.lib")

// OpenSSL
#include <openssl/ssl.h>
#include <openssl/err.h>
#include <openssl/x509_vfy.h>
#include <openssl/x509.h>

#include <iostream>
#include <cstring>   // for memset
#include <string>

////////////////////////////////////////////////////////////////
// A small RAII helper class for WinSock initialization
////////////////////////////////////////////////////////////////
class WinSockInitializer {
public:
    WinSockInitializer() {
        WSADATA wsaData;
        int result = WSAStartup(MAKEWORD(2,2), &wsaData);
        if (result != 0) {
            std::cerr << "WSAStartup failed with error: " << result << "\n";
            // In real code, handle error or throw exception
        }
    }

    ~WinSockInitializer() {
        WSACleanup();
    }
};

////////////////////////////////////////////////////////////////
// Utility Functions
////////////////////////////////////////////////////////////////

// RAII approach means we no longer need repeated WSAStartup calls.
// We'll keep a static instance so the entire program has WinSock up.
static WinSockInitializer g_winsock; 

// Read from a plain (non-TLS) socket until the connection closes.
// Note we use "SOCKET" instead of "int" in Windows.
static std::string readPlainTCP(SOCKET sock) {
    std::string response;
    char buffer[4096];

    while (true) {
        // Windows "recv" can return SOCKET_ERROR, so we check carefully
        int bytesReceived = recv(sock, buffer, sizeof(buffer) - 1, 0);
        if (bytesReceived == SOCKET_ERROR) {
            // If it's a transient error, handle it. For now, we break on error.
            std::cerr << "recv() failed with error: " << WSAGetLastError() << "\n";
            break;
        }
        if (bytesReceived <= 0) {
            // 0 means connection closed
            break;
        }

        buffer[bytesReceived] = '\0'; // null-terminate
        response += buffer;
    }
    return response;
}

// Read from a TLS/SSL connection using OpenSSL
static std::string readTLS(SSL* ssl) {
    std::string response;
    char buffer[4096];

    while (true) {
        int bytesReceived = SSL_read(ssl, buffer, sizeof(buffer) - 1);
        if (bytesReceived <= 0) {
            // 0 means closed, <0 means error
            break;
        }
        buffer[bytesReceived] = '\0';
        response += buffer;
    }
    return response;
}

////////////////////////////////////////////////////////////////
// HttpClient Implementation
////////////////////////////////////////////////////////////////

HttpClient::HttpClient() {
    // Initialize OpenSSL once per process if needed
    // (OpenSSL 1.1.0+ often calls OPENSSL_init_ssl automatically.)
    OPENSSL_init_ssl(OPENSSL_INIT_LOAD_SSL_STRINGS | OPENSSL_INIT_LOAD_CRYPTO_STRINGS, nullptr);
    // SSL_load_error_strings(); // Additional if needed
}

HttpClient::~HttpClient() {
    // Cleanup: If you had dedicated SSL structures for the entire lifetime, you'd free them here.
    // But we do ephemeral SSL_CTX per fetch, so not strictly needed.
}

void HttpClient::setCAFile(const std::string& caFilePath) {
    m_caFile = caFilePath;
}

std::string HttpClient::fetch(const std::string& url) {
    // 1) Parse the URL
    ParsedURL purl = parseURL(url);

    // Force HTTPS if not localhost
    if (!purl.isLocalhost) {
        purl.isHTTPS = true;
        purl.port = "443";
    }

    // 2) Create a WinSock (TCP) socket
    SOCKET sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (sock == INVALID_SOCKET) {
        std::cerr << "socket() failed: " << WSAGetLastError() << "\n";
        return "";
    }

    // 3) DNS resolution via getaddrinfo
    struct addrinfo hints;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family   = AF_INET;       // or AF_UNSPEC for IPv6+IPv4
    hints.ai_socktype = SOCK_STREAM;   // TCP

    struct addrinfo* res = nullptr;
    int addrErr = getaddrinfo(purl.host.c_str(), purl.port.c_str(), &hints, &res);
    if (addrErr != 0 || !res) {
        std::cerr << "getaddrinfo() failed: " << gai_strerror(addrErr) << "\n";
        closesocket(sock);
        return "";
    }

    // 4) Connect to the server
    int conn = connect(sock, res->ai_addr, static_cast<int>(res->ai_addrlen));
    freeaddrinfo(res);

    if (conn == SOCKET_ERROR) {
        std::cerr << "connect() failed: " << WSAGetLastError() << "\n";
        closesocket(sock);
        return "";
    }

    // 5) Build our minimal HTTP request
    // We'll do GET with Connection: close
    std::string request;
    request += "GET " + purl.path + " HTTP/1.1\r\n";
    request += "Host: " + purl.host + "\r\n";
    request += "User-Agent: CustomBrowser/1.0\r\n";
    request += "Connection: close\r\n\r\n";

    std::string response;

    if (!purl.isHTTPS) {
        //////////////////////////////////////////////////////
        // Plain HTTP
        //////////////////////////////////////////////////////
        int sent = send(sock, request.c_str(), static_cast<int>(request.size()), 0);
        if (sent == SOCKET_ERROR) {
            std::cerr << "send() failed: " << WSAGetLastError() << "\n";
            closesocket(sock);
            return "";
        }

        response = readPlainTCP(sock);
        closesocket(sock);
    } else {
        //////////////////////////////////////////////////////
        // TLS/HTTPS
        //////////////////////////////////////////////////////

        // 1. Create an SSL_CTX
        SSL_CTX* ctx = SSL_CTX_new(TLS_client_method());
        if (!ctx) {
            std::cerr << "SSL_CTX creation failed\n";
            closesocket(sock);
            return "";
        }

        // 2. Enforce certificate verification
        SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, nullptr);

        // 2a. Load CA file if provided
        if (!m_caFile.empty()) {
            if (SSL_CTX_load_verify_locations(ctx, m_caFile.c_str(), nullptr) != 1) {
                std::cerr << "Failed to load CA file\n";
                // We continue but we won't be able to verify properly
            }
        } else {
            // If you rely on system defaults or user might supply it at runtime
            // SSL_CTX_set_default_verify_paths(ctx);
        }

        // 3. Create an SSL object
        SSL* ssl = SSL_new(ctx);
        if (!ssl) {
            std::cerr << "SSL_new failed\n";
            SSL_CTX_free(ctx);
            closesocket(sock);
            return "";
        }

        // 4. Bind our WinSock 'sock' to the SSL object
        SSL_set_fd(ssl, static_cast<int>(sock));

        // 4a. SNI (Server Name Indication)
        SSL_set_tlsext_host_name(ssl, purl.host.c_str());

        // 5. TLS handshake
        int sslConn = SSL_connect(ssl);
        if (sslConn <= 0) {
            std::cerr << "TLS handshake failed\n";
            SSL_free(ssl);
            SSL_CTX_free(ctx);
            closesocket(sock);
            return "";
        }

        // 6. Check certificate
        long verifyResult = SSL_get_verify_result(ssl);
        if (verifyResult != X509_V_OK) {
            std::cerr << "Certificate verification failed: "
                      << X509_verify_cert_error_string(verifyResult) << "\n";
            // Real browsers would warn or block
        }

 // 7. Send the HTTP request over TLS
        int sslWrite = SSL_write(ssl, request.c_str(), static_cast<int>(request.size()));
        if (sslWrite <= 0) {
            std::cerr << "SSL_write failed\n";
            SSL_shutdown(ssl);
            SSL_free(ssl);
            SSL_CTX_free(ctx);
            closesocket(sock);
            return "";
        }

        // 8. Read the encrypted response
        response = readTLS(ssl);

        // 9. Cleanup
        SSL_shutdown(ssl);
        SSL_free(ssl);
        SSL_CTX_free(ctx);
        closesocket(sock);
    }

    // 10. Strip HTTP headers (split at first \r\n\r\n)
    size_t headerEnd = response.find("\r\n\r\n");
    if (headerEnd != std::string::npos) {
        return response.substr(headerEnd + 4);
    }
    return response;
}
