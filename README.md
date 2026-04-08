## How to use

### Pre-requisite

- Having Rust and Cargo installed

### How to install

- run in your terminal :
  - git clone https://github.com/My-sidequests/Bullang.git bullang
  - cd bullang
  - cargo build --release
  - ./target/release/bullang install
 
    *Bullang is now globally installed and ready to use !*

### How to convert a folder

  - bullang convert my_folder : allows you to convert a Bullang folder nammed my_folder in Rust.
    Optional flags can be added:
    
      - -n allows you to specify a name for the converted folder.
    If not specificed, the new folder name is the previous folder name starting with _

      - -ext allows you to specify in what language to convert, by file extension.
    If not specified, the language will be Rust

      - --out allows you to specify where to create the converted folder.
    If not specified, the new folder will be created next to the original one
        
  *Exemple : bullang convert my_folder -n new_folder -ext rs --out /user/Documents*
