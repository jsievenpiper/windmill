#include <ola/Callback.h>
#include <string>
#include "ola_smart_client.h"
#include "windmill/src/ola/dmx.rs.h"

void on_register(const ola::client::Result& result) {
  if (!result.Success()) {
    std::cout << "Failed to register universe: " << result.Error();
  }
}

Client::Client(rust::Box<ola::dmx::Bridge> rust_bridge)
  : inner{},
    bridge(std::move(rust_bridge)) { }

bool Client::setup() const {
  if (!this->inner.Setup()) {
    return false;
  }

  ola::client::OlaClient* client = this->inner.GetClient();
  ola::client::RepeatableDMXCallback* dmx_callback = ola::NewCallback(const_cast<Client*>(this), &Client::on_dmx);

  client->SetDMXCallback(dmx_callback);
  client->RegisterUniverse(this->bridge->get_universe(), ola::client::REGISTER, ola::NewSingleCallback(&on_register));

  return true;
}

void Client::run() const {
  inner.GetSelectServer()->Run();
}

void Client::on_dmx(const ola::client::DMXMetadata& metadata, const ola::DmxBuffer& buffer) {
  this->bridge->on_dmx(metadata, buffer);
}

std::unique_ptr<Client> create_client(rust::Box<ola::dmx::Bridge> bridge) {
  return std::unique_ptr<Client>(new Client(std::move(bridge)));
}
