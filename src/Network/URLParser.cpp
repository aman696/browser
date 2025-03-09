#include "Network/URLParser.h"
#include <algorithm>

static bool startsWith(const std::string& str, const std::string& prefix) {
    return str.rfind(prefix, 0) == 0;
}

ParsedURL parseURL(const std::string& url) {
    ParsedURL result;

    // 1. Default path = "/", default port = "443" (HTTPS)
    result.path = "/";
    result.port = "443";
    result.isHTTPS = true; // We'll default to HTTPS

    // 2. Check if user typed "http://" or "https://"
    //    or no scheme at all:
    std::string temp = url;
    if (startsWith(temp, "http://")) {
        result.isHTTPS = false;
        temp.erase(0, 7); // remove "http://"
        // default port for HTTP
        result.port = "80";
    } else if (startsWith(temp, "https://")) {
        result.isHTTPS = true;
        temp.erase(0, 8); // remove "https://"
        result.port = "443";
    }
    // else no scheme -> assume https

    // 3. Find first '/' -> separate host from path
    auto slashPos = temp.find('/');
    if (slashPos != std::string::npos) {
        result.path = temp.substr(slashPos); // includes '/'
        temp = temp.substr(0, slashPos);
    }

    // 4. Check if there's a colon in the host -> parse port
    auto colonPos = temp.find(':');
    if (colonPos != std::string::npos) {
        result.host = temp.substr(0, colonPos);
        result.port = temp.substr(colonPos + 1);
    } else {
        result.host = temp; // no port specified
    }

    // 5. Check if host is localhost or 127.0.0.1
    if (result.host == "localhost" || result.host == "127.0.0.1") {
        result.isLocalhost = true;
    }

    return result;
}
