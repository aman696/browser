#ifndef SECURITY_MANAGER_H
#define SECURITY_MANAGER_H

#include <string>

class SecurityManager {
public:
    SecurityManager();
    ~SecurityManager();

    void blockThirdPartyCookies();
    void enforceHTTPS();
    void preventXSS(const std::string& script);
};

#endif // SECURITY_MANAGER_H
