import React from 'react';
import { Link as RouterLink, useParams } from 'react-router-dom';
import {
    Button,
    Breadcrumb,
    BreadcrumbItem,
    BreadcrumbLink,
    Container,
    Flex,
    Text,
    Heading,
    CircularProgress,
    Center,
    Icon,
    Image,
    Divider,
    Link,
    Box,
    Menu,
    MenuButton,
    MenuList,
    MenuGroup,
    MenuItem,
} from '@chakra-ui/react';
import { ChevronRightIcon, DownloadIcon } from '@chakra-ui/icons';
import { FaFilm, FaPlayCircle } from 'react-icons/fa';
import qs from 'qs';
import { useProgramQuery, useProgramThumbnailQuery } from '../generated/graphql';
import { parseAndFormatDate } from '../utils';

type ProgramParams = {
    programId: string;
};

const Program = () => {
    const { programId } = useParams<ProgramParams>();
    const { loading, error, data } = useProgramQuery({ variables: { programId } });
    const { error: thumbError, data: thumbData } = useProgramThumbnailQuery({ variables: { programId } });
    return (
        <Container maxW="container.lg" mt="1rem">
            <Breadcrumb spacing="8px" separator={<ChevronRightIcon color="gray.500" />} mb="1.25rem">
                <BreadcrumbItem as={RouterLink} to="/programs">
                    <BreadcrumbLink>番組一覧</BreadcrumbLink>
                </BreadcrumbItem>
                <BreadcrumbItem isCurrentPage>
                    <BreadcrumbLink>番組情報</BreadcrumbLink>
                </BreadcrumbItem>
            </Breadcrumb>
            {loading ? (
                <Center flexDirection="column">
                    <CircularProgress isIndeterminate color="blue.300" />
                    <Text mt="3">読み込み中……</Text>
                </Center>
            ) : error ? (
                <Text color="red.500">{JSON.stringify(error)}</Text>
            ) : (
                <>
                    <Heading mb="4">{data?.program?.name}</Heading>
                    <Text color="gray.500" mb="2">
                        {parseAndFormatDate(data?.program?.startAt)}
                        {' - '}
                        {data?.program?.service?.name}
                    </Text>
                    <Flex alignItems="flex-start">
                        <Box flex={1}>
                            <Text>{data?.program?.description}</Text>
                            {data?.program?.extended?.map((ex) => (
                                <Text mt="2">
                                    {ex?.key}
                                    <br />
                                    {ex?.value}
                                </Text>
                            ))}
                        </Box>
                        {thumbError && <Text color="red.500">{JSON.stringify(thumbError)}</Text>}
                        {thumbData?.program?.thumbnail && (
                            <Image src={thumbData.program.thumbnail} width="320px" objectFit="contain" ml="3" />
                        )}
                    </Flex>
                    <Divider my="4" />
                    <Heading size="sm">動画一覧</Heading>
                    {data?.program?.videos &&
                        data.program.videos.map((video) => (
                            <Box key={video.id} mt="2">
                                <Menu>
                                    <MenuButton
                                        as={Button}
                                        leftIcon={<Icon as={FaPlayCircle} />}
                                        colorScheme="blue"
                                        disabled={!data?.presets || data.presets.length === 0}
                                    >
                                        視聴
                                    </MenuButton>
                                    <MenuList>
                                        <MenuGroup title="エンコードプロファイルを選択...">
                                            {data?.presets?.map((preset) => (
                                                <Link
                                                    key={preset.id}
                                                    href={`/stream?${qs.stringify({
                                                        id: video.id,
                                                        preset: preset.id,
                                                    })}`}
                                                    target="_blank"
                                                    _hover={undefined}
                                                >
                                                    <MenuItem>{preset.title || preset.id}</MenuItem>
                                                </Link>
                                            ))}
                                        </MenuGroup>
                                    </MenuList>
                                </Menu>
                                <Link href={`/stream?id=${video.id}`} target="_blank" _hover={undefined} ml="2">
                                    <Button leftIcon={<DownloadIcon />}>
                                        ダウンロード
                                        <Text color="gray.600" fontSize="xs" ml="0.5">
                                            ({(parseInt(video.totalLength, 10) / 1024 / 1024).toFixed(1)} MB)
                                        </Text>
                                    </Button>
                                </Link>
                                <Icon as={FaFilm} mx="2" />
                                {video.fileName}
                            </Box>
                        ))}
                </>
            )}
        </Container>
    );
};

export default Program;
