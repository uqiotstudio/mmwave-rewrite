# mmwave-rewrite
mmwave rewrite

# December Plan
1. Reimplement the radar reader with the following improvements:
  -[X] Connection loss handling
    - This is a really complex problem. See the device locking section.
  -[X] More stability/bug fixes
  -[X] Better config writing
  -[x] Support for all possible TLVs
  -[X] Swap from /dev/ttyACM* handles to usb serial number for reliability
  -[] Builtin pointcloud transformation
2. Set up Dora Node which utilizes radar reader and publishes Data.
  -[] If possible, let commands be sent to the node to reconfigure the radar/transform.
3. Set up Dora Node which visualizes available data.
4. Data recording/replaying.
  -[] Set up Dora Node to record data to a file.
  -[] Set up Dora Node to replay data from a file.
5. Networking of Dora nodes (Stretch Goal, probably spill into january...)

# January Plan
1. Develop data collection Protocol/Plan, organize to get it done.
2. Convert the models over to pytorch-geometric or use 3d-convolution. Ideally rewrite them to easily support multiple models for better exploration.
3. Train the model for person detection & classification (sitting/standing).
4. Create a Dora Node which runs the model and publishes results.
5. Create Dora Node to record people classification to file.
