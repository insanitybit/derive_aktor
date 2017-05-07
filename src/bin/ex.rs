pub fn info<InfoT: Debug + Send + 'static, ErrorT: Debug + Send + 'static>(&self, data: ty, ) {
    let msg = PrintLoggerMessage::InfoVariant { data: data };
    self.sender.send(msg);
}