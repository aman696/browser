#ifndef JAVASCRIPT_ENGINE_H
#define JAVASCRIPT_ENGINE_H

#include <string>

class JavaScriptEngine {
public:
    JavaScriptEngine();
    ~JavaScriptEngine();

    void execute(const std::string& script);
};

#endif // JAVASCRIPT_ENGINE_H
