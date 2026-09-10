#define DOCTEST_CONFIG_IMPLEMENT
#include <doctest/doctest.h>
#include <ixwebsocket/IXNetSystem.h>

int main(int argc, char** argv) {
    ix::initNetSystem();
    doctest::Context context;
    context.applyCommandLine(argc, argv);
    int result = context.run();
    ix::uninitNetSystem();
    return result;
}
