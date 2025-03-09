#ifndef URLPARSER_H
#define URLPARSER_H

#include <string>

struct ParsedURL {
    bool isLocalhost = false;   // true if localhost or 127.0.0.1
    bool isHTTPS = false;       // user typed https OR forced to https
    std::string host;
    std::string path;
    std::string port;           // "443" for HTTPS, "80" for HTTP
};

// Minimal parse: http://localhost:8080/path -> host=localhost, port=8080, path=/path
ParsedURL parseURL(const std::string& url);

#endif
