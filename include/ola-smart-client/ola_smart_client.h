#ifndef SIEVENPIPER_OLA_CLIENT
#define SIEVENPIPER_OLA_CLIENT

#include <ola/client/ClientWrapper.h>
#include <memory>
#include "rust/cxx.h"

namespace ola {
  namespace dmx {
    struct Bridge;
  }
}

class Client {
  private:
    mutable ola::client::OlaClientWrapper inner;
    rust::Box<ola::dmx::Bridge> bridge;
    void on_dmx(const ola::client::DMXMetadata& metadata, const ola::DmxBuffer& buffer);

  public:
    Client(rust::Box<ola::dmx::Bridge> bridge);
    bool setup() const;
    void run() const;
};

std::unique_ptr<Client> create_client(rust::Box<ola::dmx::Bridge> bridge);

#endif
