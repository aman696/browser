#include "Network/HttpClient.h"
#include <iostream>

int main() {
    HttpClient client;

    // Provide the path to your CA bundle (example path for MSYS2):
    // Adjust this path to where your CA cert file is located!
    client.setCAFile("C:\\msys64\\usr\\ssl\\certs\\ca-bundle.crt");

    // Now fetch a site:
    std::string html = client.fetch("https://www.google.com");
    // or even "http://www.google.com" (will be forced to https)

    std::cout << html << std::endl;
    return 0;
}
