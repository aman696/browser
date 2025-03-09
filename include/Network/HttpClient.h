#ifndef HTTP_CLIENT_H
#define HTTP_CLIENT_H

#include <string>

// Forward declaration
class HttpClient {
public:
    HttpClient();
    ~HttpClient();

    // Set the path to your CA certificate file (for verifying server certificates)
    void setCAFile(const std::string& caFilePath);

    // Fetch HTML content from a URL.
    // Enforces HTTPS if not localhost, using OpenSSL for TLS.
    std::string fetch(const std::string& url);

private:
    std::string m_caFile; // Path to CA bundle
};

#endif // HTTP_CLIENT_H
